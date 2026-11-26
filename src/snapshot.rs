use crate::{baseline::{self, Baseline, Kind}, collect, group, history::History, model::*, notify, rules};
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
                findings.extend(new_remote_finding(g, &hosts, &base));
            }
        }
        findings.sort_by_key(|f| f.severity);
    }
    let (findings, expected_findings) = split_expected(findings, &base);
    notify::dispatch(&findings);
    for g in groups.iter_mut() {
        let (trust, fired) = rules::classify(g, &findings);
        g.trust = trust;
        g.rules_fired = fired;
    }
    Snapshot {
        at: now,
        totals: Totals { processes: np, connections: nc, groups: groups.len(), listening, outbound, findings: findings.len() },
        groups, findings, expected_findings,
    }
}

/// The "new hosts" finding for one approved group, leaving out hosts the user marked as expected.
/// None when every new host is expected.
fn new_remote_finding(g: &Group, hosts: &[String], base: &Baseline) -> Option<Finding> {
    let hosts: Vec<&String> = hosts.iter().filter(|h| !base.has(Kind::Host, &baseline::host_key(&g.key, h))).collect();
    if hosts.is_empty() { return None; }
    let shown = hosts.iter().take(4).map(|h| h.as_str()).collect::<Vec<_>>().join(", ");
    Some(Finding { severity: Severity::Medium, group: g.label.clone(), group_key: g.key.clone(), rule: "new-remote",
        detail: format!("{} host(s) first seen in the last hour: {shown}{}", hosts.len(), if hosts.len() > 4 { ", ..." } else { "" }) })
}

/// Splits findings into (active, expected). An unsigned program using the network is never
/// expected, however it was marked: approving an app must not launder it either (`rules::classify`).
fn split_expected(findings: Vec<Finding>, base: &Baseline) -> (Vec<Finding>, Vec<Finding>) {
    findings.into_iter().partition(|f| f.rule == "unsigned-network" || !base.has(Kind::Finding, &f.id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(key: &str) -> Group {
        Group { key: key.into(), label: "X".into(), category: "Apps".into(), procs: vec![], conns: vec![], signing: None, approved: true, trust: Label::Unknown, rules_fired: vec![] }
    }
    fn finding(rule: &'static str) -> Finding {
        Finding { severity: Severity::High, group: "X".into(), group_key: "/Applications/X.app".into(), rule, detail: String::new() }
    }
    fn marked(entries: &[(Kind, &str)]) -> Baseline {
        Baseline::from_entries(entries.iter().map(|(k, key)| baseline::typed(*k, key)).collect())
    }

    #[test]
    fn an_expected_host_drops_out_of_the_new_hosts_finding_and_the_others_stay() {
        let g = group("/Applications/X.app");
        let base = marked(&[(Kind::Host, "/Applications/X.app|1.1.1.1")]);
        let f = new_remote_finding(&g, &["1.1.1.1".into(), "2.2.2.2".into()], &base).expect("2.2.2.2 is still new");
        assert!(f.detail.contains("1 host(s)") && f.detail.contains("2.2.2.2") && !f.detail.contains("1.1.1.1"), "{}", f.detail);
    }

    #[test]
    fn no_new_hosts_finding_when_every_new_host_is_expected() {
        let g = group("/Applications/X.app");
        let base = marked(&[(Kind::Host, "/Applications/X.app|1.1.1.1")]);
        assert!(new_remote_finding(&g, &["1.1.1.1".into()], &base).is_none());
    }

    #[test]
    fn a_host_expected_for_one_app_is_still_new_for_another() {
        let base = marked(&[(Kind::Host, "/Applications/Y.app|1.1.1.1")]);
        assert!(new_remote_finding(&group("/Applications/X.app"), &["1.1.1.1".into()], &base).is_some());
    }

    #[test]
    fn an_expected_finding_moves_to_the_expected_list() {
        let base = marked(&[(Kind::Finding, "/Applications/X.app:exposed-listener")]);
        let (active, expected) = split_expected(vec![finding("exposed-listener"), finding("debug-port")], &base);
        assert_eq!(active.iter().map(|f| f.rule).collect::<Vec<_>>(), vec!["debug-port"]);
        assert_eq!(expected.iter().map(|f| f.rule).collect::<Vec<_>>(), vec!["exposed-listener"]);
    }

    #[test]
    fn an_unsigned_network_finding_cannot_be_made_expected() {
        let base = marked(&[(Kind::Finding, "/Applications/X.app:unsigned-network")]);
        let (active, expected) = split_expected(vec![finding("unsigned-network")], &base);
        assert_eq!(active.len(), 1);
        assert!(expected.is_empty());
    }
}
