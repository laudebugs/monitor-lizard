use crate::{baseline, collect, group, model::*, rules};
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
    let findings = rules::evaluate(&groups);
    Snapshot {
        at: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        totals: Totals { processes: np, connections: nc, groups: groups.len(), listening, outbound, findings: findings.len() },
        groups, findings,
    }
}
