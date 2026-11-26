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
        let f = |severity, rule, detail: String| Finding { severity, group: g.label.clone(), group_key: g.key.clone(), rule, detail };
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

/// The trust label for one group and the rules that produced it, from the same findings the dashboard shows.
/// Only rules decide this: a model may later explain a label or escalate it, but never lower it.
pub fn classify(g: &Group, findings: &[Finding]) -> (Label, Vec<&'static str>) {
    let mut fired: Vec<&'static str> = Vec::new();
    let apple = g.signing.as_ref().is_some_and(|s| s.apple);
    if apple { fired.push("apple-signed"); }
    if g.approved { fired.push("approved"); }
    for f in findings.iter().filter(|f| f.group_key == g.key) {
        if !fired.contains(&f.rule) { fired.push(f.rule); }
    }
    // An unsigned program using the network stays suspicious even after you approved it.
    let label = if fired.contains(&"unsigned-network") { Label::Suspicious }
        // Apple-signed wins over every other finding: Apple's own LAN services and tools are expected.
        else if apple { Label::Trusted }
        else if g.approved && fired.iter().all(|r| *r == "approved") { Label::Trusted }
        // Approved but flagged again, or not approved and not known to be Apple's.
        else { Label::Unknown };
    // Every unknown label must say why: when no finding does, the reason is that nothing verifies it.
    if label == Label::Unknown && fired.is_empty() { fired.push("unverified"); }
    (label, fired)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(direction: Direction, scope: Scope, exposed: bool, port: u16) -> Conn {
        Conn { pid: 1, name: "x".into(), proto: "TCP".into(), state: "ESTABLISHED".into(), local_addr: "10.0.0.2".into(), local_port: port,
            remote_addr: None, remote_port: None, direction, scope, exposed }
    }
    /// A group with the given trust facts: approval, signing (None = not checked), and its sockets.
    fn grp(approved: bool, signing: Option<(bool, bool)>, conns: Vec<Conn>) -> Group {
        Group { key: "/Applications/X.app".into(), label: "X".into(), category: "Apps".into(), procs: vec![], conns,
            signing: signing.map(|(signed, apple)| Signing { signed, team: None, authority: None, apple }), approved,
            trust: Label::Unknown, rules_fired: vec![] }
    }
    fn net() -> Vec<Conn> { vec![conn(Direction::Outbound, Scope::Public, false, 50000)] }
    fn label(g: &Group, extra: &[&'static str]) -> (Label, Vec<&'static str>) {
        let mut findings = evaluate(std::slice::from_ref(g));
        for r in extra { findings.push(Finding { severity: Severity::Medium, group: g.label.clone(), group_key: g.key.clone(), rule: r, detail: String::new() }); }
        classify(g, &findings)
    }

    #[test]
    fn unsigned_network_is_suspicious_approved_or_not() {
        assert_eq!(label(&grp(false, Some((false, false)), net()), &[]).0, Label::Suspicious);
        // regression: approving must not launder an unsigned networked program
        let (l, fired) = label(&grp(true, Some((false, false)), net()), &[]);
        assert_eq!(l, Label::Suspicious);
        assert!(fired.contains(&"unsigned-network") && fired.contains(&"approved"));
    }
    #[test]
    fn unapproved_signed_third_party_with_network_is_unknown() {
        let (l, fired) = label(&grp(false, Some((true, false)), net()), &[]);
        assert_eq!(l, Label::Unknown);
        assert!(fired.contains(&"unreviewed"));
    }
    #[test]
    fn approved_with_exposed_listener_or_new_remote_is_unknown_again() {
        let exposed = vec![conn(Direction::Listen, Scope::Loopback, true, 8080)];
        assert_eq!(label(&grp(true, Some((true, false)), exposed), &[]).0, Label::Unknown);
        assert_eq!(label(&grp(true, Some((true, false)), net()), &["new-remote"]).0, Label::Unknown);
    }
    #[test]
    fn approved_with_a_debug_port_is_unknown() {
        let dbg = vec![conn(Direction::Listen, Scope::Loopback, false, 9222)];
        assert_eq!(label(&grp(true, Some((true, false)), dbg), &[]).0, Label::Unknown);
    }
    #[test]
    fn apple_signed_stays_trusted_even_with_a_debug_port() {
        // Decided, not accidental: Apple-signed wins over medium findings. Revisit if an Apple tool ever opens 9222.
        let dbg = vec![conn(Direction::Listen, Scope::Loopback, false, 9222)];
        let (l, fired) = label(&grp(false, Some((true, true)), dbg), &[]);
        assert_eq!(l, Label::Trusted);
        assert!(fired.contains(&"debug-port"));
    }
    #[test]
    fn approved_and_clean_is_trusted() {
        let (l, fired) = label(&grp(true, Some((true, false)), net()), &[]);
        assert_eq!(l, Label::Trusted);
        assert_eq!(fired, vec!["approved"]);
    }
    #[test]
    fn apple_signed_is_trusted_even_when_reachable_from_the_lan() {
        let lan = vec![conn(Direction::Listen, Scope::Loopback, true, 7000)];
        let (l, fired) = label(&grp(false, Some((true, true)), lan), &[]);
        assert_eq!(l, Label::Trusted);
        assert!(fired.contains(&"apple-signed") && fired.contains(&"exposed-listener-apple"));
    }
    #[test]
    fn no_sockets_is_unknown_unless_apple_or_approved() {
        let (l, fired) = label(&grp(false, None, vec![]), &[]);
        assert_eq!((l, fired), (Label::Unknown, vec!["unverified"]), "an unknown label always names its reason");
        assert_eq!(label(&grp(true, None, vec![]), &[]).0, Label::Trusted);
        assert_eq!(label(&grp(false, Some((true, true)), vec![]), &[]).0, Label::Trusted);
    }
    #[test]
    fn findings_of_other_groups_do_not_leak_into_a_label() {
        let g = grp(true, Some((true, false)), net());
        let other = Finding { severity: Severity::High, group: "Y".into(), group_key: "/Applications/Y.app".into(), rule: "unsigned-network", detail: String::new() };
        assert_eq!(classify(&g, &[other]).0, Label::Trusted);
    }
}
