use crate::{baseline, collect, group, history::History, model::*, notify, rules};
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
    let mut groups = group::build(procs, conns, &baseline::load());
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
                let shown = hosts.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
                findings.push(Finding { severity: Severity::Medium, group: g.label.clone(), group_key: g.key.clone(), rule: "new-remote",
                    detail: format!("{} host(s) first seen in the last hour: {shown}{}", hosts.len(), if hosts.len() > 4 { ", ..." } else { "" }) });
            }
        }
        findings.sort_by_key(|f| f.severity);
    }
    notify::dispatch(&findings);
    for g in groups.iter_mut() {
        let (trust, fired) = rules::classify(g, &findings);
        g.trust = trust;
        g.rules_fired = fired;
    }
    Snapshot {
        at: now,
        totals: Totals { processes: np, connections: nc, groups: groups.len(), listening, outbound, findings: findings.len() },
        groups, findings,
    }
}
