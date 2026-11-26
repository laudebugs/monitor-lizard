use crate::{baseline::{self, Baseline, Kind}, collect, group, history::History, model::*, notify, rules};
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn take() -> Snapshot {
    let (procs, conns) = std::thread::scope(|s| {
        let p = s.spawn(collect::processes);
        let c = s.spawn(collect::connections);
        (p.join().unwrap(), c.join().unwrap())
    });
    let (np, nc) = (procs.len(), conns.len());
    let (listening, outbound) = (
        conns.iter().filter(|c| c.direction == Direction::Listen).count(),
        conns.iter().filter(|c| c.direction == Direction::Outbound).count(),
    );
    let base = baseline::load();
    let mut groups = group::build(procs, conns, &base.apps());
    // codesign is slow: only verify groups that touch the network, in parallel.
    std::thread::scope(|s| {
        for g in groups.iter_mut().filter(|g| !g.conns.is_empty()) {
            s.spawn(move || {
                let target = if g.key.ends_with(".app") { g.key.clone() } else { g.procs[0].exe.clone() };
                if target.starts_with('/') { g.signing = Some(collect::signing_of(&target)); }
            });
        }
    });
    let mut findings = rules::evaluate(&groups);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if let Ok(mut h) = History::open() {
        let _ = h.record(&groups, now);
        // Only approved apps: for unapproved ones every host is new and "unreviewed" already covers it.
        for (key, hosts) in h.recent_new(now).unwrap_or_default() {
            if let Some(g) = groups.iter().find(|g| g.key == key && g.approved) {
                // Unfiltered: marking hosts expected must not lift the label, so the label is computed from every host.
                findings.extend(new_remote_finding(g, &hosts, &Baseline::default()));
            }
        }
        findings.sort_by_key(|f| f.severity);
    }
    for g in groups.iter_mut() {
        g.expected_hosts = g.conns.iter().filter_map(|c| c.remote_addr.as_deref()).filter(|a| base.has(Kind::Host, &baseline::host_key(&g.key, a))).map(String::from).collect::<BTreeSet<_>>().into_iter().collect();
    }
    let (findings, expected_findings) = label_and_split(&mut groups, findings, &base);
    notify::dispatch(&findings);
    Snapshot {
        at: now,
        totals: Totals { processes: np, connections: nc, groups: groups.len(), listening, outbound, findings: findings.len() },
        groups, findings, expected_findings, baseline_notice: baseline::notice(),
    }
}

/// The "new hosts" finding for one approved group, leaving out hosts the user marked as expected.
/// None when every new host is expected. The evidence is the hosts, so the UI can mark them one by one.
fn new_remote_finding(g: &Group, hosts: &[String], base: &Baseline) -> Option<Finding> {
    let hosts: Vec<&String> = hosts.iter().filter(|h| !base.has(Kind::Host, &baseline::host_key(&g.key, h))).collect();
    if hosts.is_empty() { return None; }
    let shown = hosts.iter().take(4).map(|h| h.as_str()).collect::<Vec<_>>().join(", ");
    let evidence = hosts.iter().map(|h| h.as_str()).collect::<Vec<_>>().join(",");
    Some(Finding::new(Severity::Medium, g.label.clone(), g.key.clone(), "new-remote",
        format!("{} host(s) first seen in the last hour: {shown}{}", hosts.len(), if hosts.len() > 4 { ", ..." } else { "" }), evidence))
}

/// Labels every group from ALL findings, then splits them into (active, expected). Marking a
/// finding as expected changes what is shown, counted and notified, never the trust label: a label
/// is decided by rules only, and nothing the user clicks may raise it. The two never-expected rules
/// stay active even if a key for them is in the file.
fn label_and_split(groups: &mut [Group], findings: Vec<Finding>, base: &Baseline) -> (Vec<Finding>, Vec<Finding>) {
    for g in groups.iter_mut() {
        let (trust, fired) = rules::classify(g, &findings);
        g.trust = trust;
        g.rules_fired = fired;
    }
    let mut active = Vec::new();
    let mut expected = Vec::new();
    for f in findings {
        if f.mark_key.as_ref().is_some_and(|k| base.has(Kind::Finding, k)) { expected.push(f); continue; }
        if f.rule == "new-remote" {
            let Some(g) = groups.iter().find(|g| g.key == f.group_key) else { active.push(f); continue };
            let hosts: Vec<String> = f.evidence.split(',').map(String::from).collect();
            match new_remote_finding(g, &hosts, base) { Some(shown) => active.push(shown), None => expected.push(f) }
            continue;
        }
        active.push(f);
    }
    (active, expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(key: &str, approved: bool) -> Group {
        Group { key: key.into(), label: "X".into(), category: "Apps".into(), procs: vec![], conns: vec![], signing: Some(Signing { signed: true, team: None, authority: None, apple: false }), approved, trust: Label::Unknown, rules_fired: vec![], expected_hosts: vec![] }
    }
    fn exposed(ports: &str) -> Finding {
        Finding::new(Severity::High, "X".into(), "/Applications/X.app".into(), "exposed-listener", String::new(), ports.into())
    }
    fn marked(entries: &[(Kind, String)]) -> Baseline {
        Baseline::from_entries(entries.iter().map(|(k, key)| baseline::typed(*k, key)).collect())
    }
    fn mark_of(f: &Finding) -> (Kind, String) { (Kind::Finding, f.mark_key.clone().expect("markable")) }

    #[test]
    fn an_expected_host_drops_out_of_the_new_hosts_finding_and_the_others_stay() {
        let g = group("/Applications/X.app", true);
        let base = marked(&[(Kind::Host, "/Applications/X.app|1.1.1.1".into())]);
        let f = new_remote_finding(&g, &["1.1.1.1".into(), "2.2.2.2".into()], &base).expect("2.2.2.2 is still new");
        assert!(f.detail.contains("1 host(s)") && f.detail.contains("2.2.2.2") && !f.detail.contains("1.1.1.1"), "{}", f.detail);
    }

    #[test]
    fn no_new_hosts_finding_when_every_new_host_is_expected() {
        let base = marked(&[(Kind::Host, "/Applications/X.app|1.1.1.1".into())]);
        assert!(new_remote_finding(&group("/Applications/X.app", true), &["1.1.1.1".into()], &base).is_none());
    }

    #[test]
    fn a_host_expected_for_one_app_is_still_new_for_another() {
        let base = marked(&[(Kind::Host, "/Applications/Y.app|1.1.1.1".into())]);
        assert!(new_remote_finding(&group("/Applications/X.app", true), &["1.1.1.1".into()], &base).is_some());
    }

    #[test]
    fn marking_a_finding_moves_it_out_of_the_active_list_but_never_raises_the_label() {
        let f = exposed("3000");
        let mut unmarked = vec![group("/Applications/X.app", true)];
        let (active, _) = label_and_split(&mut unmarked, vec![exposed("3000")], &Baseline::default());
        assert_eq!(active.len(), 1);
        let before = unmarked[0].trust;
        assert_eq!(before, Label::Unknown, "an approved app with an exposed listener is Unknown");

        let mut with_mark = vec![group("/Applications/X.app", true)];
        let (active, expected) = label_and_split(&mut with_mark, vec![exposed("3000")], &marked(&[mark_of(&f)]));
        assert!(active.is_empty() && expected.len() == 1);
        assert_eq!(with_mark[0].trust, before, "the label is the same with the finding marked");
        assert!(with_mark[0].rules_fired.contains(&"exposed-listener"), "the rule that fired is still listed");
    }

    #[test]
    fn marking_hosts_expected_does_not_raise_the_label_either() {
        let hosts = ["1.1.1.1".to_string()];
        let g = group("/Applications/X.app", true);
        let full = || new_remote_finding(&g, &hosts, &Baseline::default()).unwrap();
        let mut a = vec![group("/Applications/X.app", true)];
        label_and_split(&mut a, vec![full()], &Baseline::default());
        let mut b = vec![group("/Applications/X.app", true)];
        let (active, expected) = label_and_split(&mut b, vec![full()], &marked(&[(Kind::Host, "/Applications/X.app|1.1.1.1".into())]));
        assert!(active.is_empty() && expected.len() == 1);
        assert_eq!(a[0].trust, b[0].trust);
    }

    #[test]
    fn a_mark_covers_only_the_ports_it_was_made_on() {
        let three = exposed("3000");
        let base = marked(&[mark_of(&three)]);
        let mut gs = vec![group("/Applications/X.app", true)];
        let (active, _) = label_and_split(&mut gs, vec![exposed("3000,3001")], &base);
        assert_eq!(active.len(), 1, "a second port brings the finding back");
        let both = exposed("3000,3001");
        assert_ne!(three.mark_key, both.mark_key);
        let base = marked(&[mark_of(&both)]);
        let (active, expected) = label_and_split(&mut gs, vec![exposed("3000,3001")], &base);
        assert!(active.is_empty() && expected.len() == 1, "marking the new set hides it again");
    }

    #[test]
    fn findings_that_can_never_be_expected_carry_no_mark_key() {
        for rule in ["unsigned-network", "debug-port", "unreviewed", "new-remote"] {
            let f = Finding::new(Severity::High, "X".into(), "/Applications/X.app".into(), rule, String::new(), "1.1.1.1".into());
            assert!(f.mark_key.is_none(), "{rule}");
        }
    }
}
