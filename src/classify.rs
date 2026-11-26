//! What the agent is allowed to suggest, and the bounded evidence it may see: the model never
//! decides either for itself. `allowed_actions` is deliberately a single function so the policy
//! lives in one place; `evidence` is a snapshot of what the dashboard already shows, capped so a
//! malicious program name or command line cannot blow out the prompt.
//!
//! This module's name sits next to `rules::classify`, which is a different thing: that one decides
//! a group's trust *label* from its signals; this one decides what the label *allows*, and builds
//! the evidence record, once a label already exists.
use crate::{history::History, llm, model::{Direction, Group, Label}, persistence, snapshot};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Approve, Ignore, Investigate, Quit, ForceQuit,
    /// Named here so a consumer can match on it and so the "never allowed" rule is explicit in the
    /// type, not just in `allowed_actions`' behaviour; no network extension exists yet to act on it.
    #[allow(dead_code)]
    BlockNetwork,
}

/// The set of actions a group's rule label allows the agent to suggest. Never includes
/// `BlockNetwork`: no network extension exists yet, so there is nothing to allow it to do.
/// `Quit`/`ForceQuit` are dropped when nothing in the group could actually be signalled, so the
/// agent never suggests a button that `actions::execute` would refuse outright -- checked through
/// `actions::check` itself (a live `ps` lookup per pid), not a separate, possibly-stale copy of its
/// policy, so the two can never drift apart.
pub fn allowed_actions(label: Label, group: &Group) -> Vec<Action> {
    let my_uid = crate::actions::my_uid();
    allowed_actions_for(label, group, |pid| crate::actions::check(pid, &my_uid).is_ok())
}

fn allowed_actions_for(label: Label, group: &Group, is_signalable: impl Fn(u32) -> bool) -> Vec<Action> {
    use Action::*;
    let signalable = group.procs.iter().any(|p| is_signalable(p.pid));
    match label {
        Label::Trusted if group.approved => vec![Ignore],
        Label::Trusted => vec![Approve, Ignore],
        Label::Unknown => {
            let mut v = vec![Approve, Ignore, Investigate];
            if signalable { v.push(Quit); }
            v
        }
        // ForceQuit is reserved for Suspicious: the one label where an immediate, non-graceful
        // signal might be worth the risk of killing something mid-write. Trusted and Unknown only
        // ever get the graceful Quit, if anything.
        Label::Suspicious => {
            let mut v = vec![Investigate];
            if signalable { v.push(Quit); v.push(ForceQuit); }
            v
        }
    }
}

/// Cut to a prompt-safe length and strip control characters (a process can name itself almost
/// anything), so one group's data can never blow out the token budget or inject raw control bytes.
fn cap(s: &str) -> String { cap_len(s, 256) }

fn cap_len(s: &str, n: usize) -> String {
    let clean: String = s.chars().filter(|c| !c.is_control()).collect();
    clean.chars().take(n).collect()
}

// Same figure `llm.rs` already enforces on length alone; not imported from there (it's private, and
// this module's own cap -- length and control characters both -- should hold regardless of whatever
// partial cleaning a particular model or server did or didn't do upstream.
const RATIONALE_MAX: usize = 240;

/// The agent's suggestion once it has been checked against the rules: never the model's raw reply.
/// Kept distinct from `llm::Verdict` (what the model actually said, still untrusted) on purpose --
/// a value of this type can only come from `validate_verdict`, so nothing downstream can display or
/// act on a verdict that skipped the checks below. Markup in `rationale` is left as plain text, not
/// escaped: this module has no notion of a display surface, so escaping it here would be guessing
/// at a format that whatever shows it (A15) may not even use.
// No caller outside the tests below yet: showing a Verdict is a later todo's job (A15).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Verdict {
    pub label: Label,
    pub confidence: llm::Confidence,
    pub actions: Vec<Action>,
    pub rationale: String,
}

/// Turns a raw, untrusted reply (or the lack of one) into a `Verdict` that can never exceed what
/// the rules already decided: `label` can only move toward `Suspicious` from `rule_label`, never
/// away from it, and every action is first filtered to `allowed` -- the same list `allowed_actions`
/// built from `rule_label`, passed in rather than recomputed here so this function stays pure and
/// ignorant of how to reach a `Group`. A reply that failed to arrive or failed to parse
/// (`Err(LlmError::Unavailable(_) | LlmError::Invalid(_))`) is treated as no suggestion at all: the
/// rule label stands alone, actions fall back to whatever of `Investigate` the rules themselves
/// already allow (never inserted if the rules didn't allow it, so this fallback can't be the one
/// case that violates the "never outside `allowed`" guarantee either).
#[allow(dead_code)] // no caller outside the tests below yet: wiring A8's result through this is A15's job
pub fn validate_verdict(rule_label: Label, allowed: &[Action], verdict: Result<llm::Verdict, llm::LlmError>) -> Verdict {
    let Ok(raw) = verdict else {
        let actions = if allowed.contains(&Action::Investigate) { vec![Action::Investigate] } else { vec![] };
        return Verdict { label: rule_label, confidence: llm::Confidence::Low, actions, rationale: String::new() };
    };
    let label = rule_label.max(raw.label); // Label's declared order (Trusted < Unknown < Suspicious) is its severity order
    // Order-preserving, not just adjacent-duplicate removal (Vec::dedup): the model's own reply
    // order isn't something to rely on being free of repeats in any particular pattern.
    let mut actions: Vec<Action> = Vec::new();
    for a in raw.actions {
        if allowed.contains(&a) && !actions.contains(&a) { actions.push(a); }
    }
    if actions.is_empty() && allowed.contains(&Action::Investigate) { actions.push(Action::Investigate); }
    Verdict { label, confidence: raw.confidence, actions, rationale: cap_len(&raw.rationale, RATIONALE_MAX) }
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Evidence {
    pub group_key: String,
    pub label: String,
    pub path_class: String,
    pub signed: bool,
    pub apple: bool,
    pub team_id: Option<String>,
    /// Whether any process in the group was launched directly by launchd (ppid 1): a background
    /// service or agent rather than something the user opened by hand.
    pub launched_by_launchd: bool,
    pub listening_ports: Vec<u16>,
    pub remote_host_count: usize,
    /// Seconds since this group's first outbound connection was recorded, if it has ever made one.
    pub first_seen_age_secs: Option<u64>,
    pub persistent: bool,
    pub trust: Label,
    pub rules_fired: Vec<&'static str>,
    pub approved: bool,
}

/// Builds the evidence record from a group plus the two facts that need a lookup elsewhere
/// (history and persistence). Pure and the part under test; `evidence` below is the thin, IO-doing
/// wrapper that gathers those two facts from a fresh snapshot.
fn build_evidence(g: &Group, first_seen_age_secs: Option<u64>, persistent: bool) -> Evidence {
    let mut ports: Vec<u16> = g.conns.iter().filter(|c| c.direction == Direction::Listen).map(|c| c.local_port).collect();
    ports.sort_unstable(); ports.dedup();
    let mut hosts: Vec<&str> = g.conns.iter().filter_map(|c| c.remote_addr.as_deref()).collect();
    hosts.sort_unstable(); hosts.dedup();

    Evidence {
        group_key: cap(&g.key),
        label: cap(&g.label),
        path_class: cap(&g.category),
        signed: g.signing.as_ref().is_some_and(|s| s.signed),
        apple: g.signing.as_ref().is_some_and(|s| s.apple),
        team_id: g.signing.as_ref().and_then(|s| s.team.as_deref()).map(cap),
        launched_by_launchd: g.procs.iter().any(|p| p.ppid == 1),
        listening_ports: ports,
        remote_host_count: hosts.len(),
        first_seen_age_secs,
        persistent,
        trust: g.trust,
        rules_fired: g.rules_fired.clone(),
        approved: g.approved,
    }
}

/// Everything about one group that the dashboard already shows, bounded and quoted for a prompt.
/// `None` if the group does not exist in a fresh snapshot (it quit, or the key was never real).
pub fn evidence(group_key: &str) -> Option<Evidence> {
    let snap = snapshot::take();
    let g = snap.groups.iter().find(|g| g.key == group_key)?;
    let first_seen_age_secs = History::open().ok().and_then(|h| h.first_seen(group_key).ok().flatten()).map(|fs| snap.at.saturating_sub(fs));
    let persistent = persistence::take().iter().any(|i| g.procs.iter().any(|p| p.exe == i.program) || i.program.starts_with(&g.key));
    Some(build_evidence(g, first_seen_age_secs, persistent))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Conn, Proc, Scope, Signing};

    fn proc(pid: u32, ppid: u32, user: &str) -> Proc {
        Proc { pid, ppid, user: user.into(), cpu: 0.0, rss_kb: 0, command: "x".into(), exe: "/Applications/X.app/x".into() }
    }
    fn conn(direction: Direction, local_port: u16, remote: Option<&str>) -> Conn {
        Conn { pid: 1, name: "x".into(), proto: "tcp".into(), state: "x".into(), local_addr: "0.0.0.0".into(),
               local_port, remote_addr: remote.map(String::from), remote_port: None, direction, scope: Scope::Lan, exposed: false }
    }
    fn group(approved: bool, procs: Vec<Proc>, conns: Vec<Conn>) -> Group {
        Group { key: "k".into(), label: "X".into(), category: "Apps".into(), procs, conns,
                signing: Some(Signing::default()), approved, trust: Label::Unknown, rules_fired: vec![] }
    }

    fn raw(label: Label, actions: Vec<Action>, rationale: &str) -> Result<llm::Verdict, llm::LlmError> {
        Ok(llm::Verdict { label, confidence: llm::Confidence::High, actions, rationale: rationale.into() })
    }

    #[test]
    fn model_says_trusted_for_a_suspicious_rule_label_is_clamped_to_suspicious() {
        let v = validate_verdict(Label::Suspicious, &[Action::Investigate], raw(Label::Trusted, vec![Action::Investigate], "fine"));
        assert_eq!(v.label, Label::Suspicious);
    }

    #[test]
    fn model_says_suspicious_for_a_trusted_rule_label_is_kept_escalation_is_allowed() {
        let v = validate_verdict(Label::Trusted, &[Action::Ignore], raw(Label::Suspicious, vec![Action::Ignore], "escalating"));
        assert_eq!(v.label, Label::Suspicious);
    }

    #[test]
    fn an_action_outside_the_allowed_set_is_dropped_and_investigate_fills_in_if_still_allowed() {
        let allowed = [Action::Investigate, Action::Quit];
        let v = validate_verdict(Label::Unknown, &allowed, raw(Label::Unknown, vec![Action::Approve, Action::ForceQuit], "x"));
        assert_eq!(v.actions, vec![Action::Investigate], "both suggested actions were outside `allowed`; Investigate fills the gap because it IS allowed here");
    }

    #[test]
    fn every_surviving_action_is_itself_one_of_the_allowed_actions() {
        let allowed = [Action::Investigate, Action::Quit];
        let v = validate_verdict(Label::Unknown, &allowed, raw(Label::Unknown, vec![Action::Approve, Action::Quit, Action::ForceQuit], "x"));
        assert!(v.actions.iter().all(|a| allowed.contains(a)));
        assert_eq!(v.actions, vec![Action::Quit]);
    }

    #[test]
    fn no_allowed_action_survives_and_investigate_itself_is_not_allowed_actions_is_left_empty() {
        // Trusted-and-approved only ever allows Ignore (see allowed_actions_for); Investigate is
        // never in it, so the fallback must not insert an action the rules never allowed, even
        // though the edge-case table's other rows show Investigate filling the gap when it IS allowed.
        let allowed = [Action::Ignore];
        let v = validate_verdict(Label::Trusted, &allowed, raw(Label::Trusted, vec![Action::Quit], "x"));
        assert!(v.actions.is_empty());
    }

    #[test]
    fn a_rationale_over_240_characters_or_with_control_characters_is_truncated_and_cleaned() {
        let long = "a\nb\x07".to_string() + &"x".repeat(1000);
        let v = validate_verdict(Label::Unknown, &[Action::Investigate], raw(Label::Unknown, vec![Action::Investigate], &long));
        assert_eq!(v.rationale.chars().count(), 240);
        assert!(!v.rationale.contains('\n') && !v.rationale.contains('\x07'));
    }

    #[test]
    fn rationale_markup_is_left_as_plain_text_unescaped() {
        let v = validate_verdict(Label::Unknown, &[Action::Investigate], raw(Label::Unknown, vec![Action::Investigate], "<script>alert(1)</script>"));
        assert_eq!(v.rationale, "<script>alert(1)</script>");
    }

    #[test]
    fn a8_unavailable_or_invalid_falls_back_to_the_rule_label_alone_with_investigate_as_the_sole_action() {
        let allowed = [Action::Investigate, Action::Quit, Action::ForceQuit];
        for err in [llm::LlmError::Unavailable("down".into()), llm::LlmError::Invalid("bad json".into()), llm::LlmError::Timeout] {
            let v = validate_verdict(Label::Suspicious, &allowed, Err(err));
            assert_eq!(v.label, Label::Suspicious);
            assert_eq!(v.actions, vec![Action::Investigate]);
            assert_eq!(v.rationale, "");
        }
    }

    #[test]
    fn duplicate_actions_from_the_model_are_not_duplicated_in_the_verdict() {
        let allowed = [Action::Investigate, Action::Quit];
        let v = validate_verdict(Label::Unknown, &allowed, raw(Label::Unknown, vec![Action::Quit, Action::Quit, Action::Investigate], "x"));
        assert_eq!(v.actions, vec![Action::Quit, Action::Investigate]);
    }

    #[test]
    fn suspicious_with_own_user_processes_gets_investigate_quit_and_force_quit() {
        let g = group(false, vec![proc(100, 1, "me")], vec![]);
        let a = allowed_actions_for(Label::Suspicious, &g, |_| true);
        assert_eq!(a, vec![Action::Investigate, Action::Quit, Action::ForceQuit]);
    }

    #[test]
    fn suspicious_with_only_root_processes_gets_investigate_only() {
        let g = group(false, vec![proc(100, 1, "root")], vec![]);
        assert_eq!(allowed_actions_for(Label::Suspicious, &g, |_| false), vec![Action::Investigate]);
    }

    #[test]
    fn unknown_gets_approve_ignore_investigate_and_quit_when_signalable() {
        let g = group(false, vec![proc(100, 1, "me")], vec![]);
        assert_eq!(allowed_actions_for(Label::Unknown, &g, |_| true), vec![Action::Approve, Action::Ignore, Action::Investigate, Action::Quit]);
    }

    #[test]
    fn trusted_and_not_approved_gets_approve_and_ignore() {
        let g = group(false, vec![proc(100, 1, "me")], vec![]);
        assert_eq!(allowed_actions_for(Label::Trusted, &g, |_| true), vec![Action::Approve, Action::Ignore]);
    }

    #[test]
    fn trusted_and_approved_gets_ignore_only() {
        let g = group(true, vec![proc(100, 1, "me")], vec![]);
        assert_eq!(allowed_actions_for(Label::Trusted, &g, |_| true), vec![Action::Ignore]);
    }

    #[test]
    fn block_network_is_never_present_for_any_label() {
        let own = group(false, vec![proc(100, 1, "me")], vec![]);
        for label in [Label::Trusted, Label::Unknown, Label::Suspicious] {
            assert!(!allowed_actions_for(label, &own, |_| true).contains(&Action::BlockNetwork));
        }
    }

    #[test]
    fn quit_and_force_quit_absent_when_every_process_is_root_owned() {
        let g = group(false, vec![proc(10, 1, "root"), proc(11, 10, "root")], vec![]);
        let a = allowed_actions_for(Label::Suspicious, &g, |_| false);
        assert!(!a.contains(&Action::Quit) && !a.contains(&Action::ForceQuit));
        let u = allowed_actions_for(Label::Unknown, &g, |_| false);
        assert!(!u.contains(&Action::Quit));
    }

    #[test]
    fn allowed_actions_asks_is_signalable_once_per_process_in_the_group() {
        // Exercises the real wiring: a group of two pids, only one of which the closure allows.
        let g = group(false, vec![proc(10, 1, "me"), proc(11, 1, "other")], vec![]);
        let a = allowed_actions_for(Label::Unknown, &g, |pid| pid == 10);
        assert!(a.contains(&Action::Quit));
    }

    #[test]
    fn allowed_actions_wires_the_real_actions_check_and_still_refuses_launchd_and_self() {
        // No fake closure here: exercises the production path, `allowed_actions` -> `actions::check`,
        // the same shared function `actions::execute` itself calls, so the two can never drift apart.
        let g = group(false, vec![proc(1, 0, "x"), proc(std::process::id(), 1, "x")], vec![]);
        assert_eq!(allowed_actions(Label::Unknown, &g), vec![Action::Approve, Action::Ignore, Action::Investigate]);
    }

    #[test]
    fn a_process_name_with_newlines_or_very_long_is_capped_and_cleaned() {
        let long = "a\nb\x07".to_string() + &"x".repeat(10_000);
        let c = cap(&long);
        assert_eq!(c.chars().count(), 256);
        assert!(!c.contains('\n') && !c.contains('\x07'));
    }

    #[test]
    fn evidence_caps_every_string_field_and_counts_ports_and_hosts() {
        let g = group(false, vec![proc(100, 50, "me")], vec![
            conn(Direction::Listen, 4747, None),
            conn(Direction::Listen, 4747, None), // duplicate: still one port
            conn(Direction::Outbound, 0, Some("1.2.3.4")),
            conn(Direction::Outbound, 0, Some("1.2.3.4")), // duplicate: still one host
            conn(Direction::Outbound, 0, Some("5.6.7.8")),
        ]);
        let e = build_evidence(&g, Some(3600), true);
        assert_eq!(e.listening_ports, vec![4747]);
        assert_eq!(e.remote_host_count, 2);
        assert!(e.persistent);
        assert_eq!(e.first_seen_age_secs, Some(3600));
        assert!(!e.launched_by_launchd);
    }

    #[test]
    fn launched_by_launchd_is_true_when_any_process_has_ppid_1() {
        let g = group(false, vec![proc(100, 1, "me")], vec![]);
        assert!(build_evidence(&g, None, false).launched_by_launchd);
    }

    #[test]
    fn a_capped_group_key_is_still_256_characters() {
        let mut g = group(false, vec![], vec![]);
        g.key = "k".repeat(10_000);
        g.label = "l".repeat(10_000);
        let e = build_evidence(&g, None, false);
        assert_eq!(e.group_key.chars().count(), 256);
        assert_eq!(e.label.chars().count(), 256);
    }
}
