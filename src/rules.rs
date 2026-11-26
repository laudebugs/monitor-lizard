use crate::model::*;
use std::collections::BTreeSet;

fn ports(g: &Group, f: impl Fn(&Conn) -> bool) -> String {
    g.conns.iter().filter(|c| f(c)).map(|c| c.local_port).collect::<BTreeSet<_>>().iter().map(u16::to_string).collect::<Vec<_>>().join(", ")
}

/// Each rule answers "is this plausibly mine?". Keep them small and explainable.
pub fn evaluate(groups: &[Group]) -> Vec<Finding> {
    let mut out = Vec::new();
    for g in groups {
        let apple = g.signing.as_ref().is_some_and(|s| s.apple);
        let f = |severity, rule, detail: String| Finding { severity, group: g.label.clone(), rule, detail };
        if g.conns.iter().any(|c| c.exposed) {
            let p = ports(g, |c| c.exposed);
            out.push(if apple { f(Severity::Low, "exposed-listener-apple", format!("Apple service reachable from LAN: {p}")) }
                     else { f(Severity::High, "exposed-listener", format!("Listening on all interfaces: {p}")) });
        }
        let net = g.conns.iter().any(|c| c.scope != Scope::Loopback);
        if net && g.signing.as_ref().is_some_and(|s| !s.signed) {
            out.push(f(Severity::High, "unsigned-network", "Unsigned binary talking to the network".into()));
        }
        if net && !g.approved && !apple {
            out.push(f(Severity::Medium, "unreviewed", "Network activity from a group you haven't approved yet".into()));
        }
        if g.conns.iter().any(|c| c.local_port == 9222 && c.direction == Direction::Listen) {
            out.push(f(Severity::Medium, "debug-port", "Remote-debugging port 9222 open: any local process can drive this browser".into()));
        }
    }
    out.sort_by_key(|f| f.severity);
    out
}
