//! Native macOS notifications for high-severity findings, off by default. `osascript` is invoked
//! with `on run argv` so the title and body are always passed as process arguments, never built
//! into the script text: the only safe way to hand it untrusted strings (a process name, a path).
use crate::{
    history::History,
    model::{Finding, Severity},
    settings::{self, Level, Settings},
};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const DEDUPE_SECS: u64 = 24 * 3600;
const SCRIPT: &str = "on run argv\n  display notification (item 2 of argv) with title (item 1 of argv)\nend run";

/// Pure: the exact argv `send` runs, so this is what injection safety is tested against. Title and
/// body are always separate elements here, never concatenated into `SCRIPT` -- that is the whole
/// safety property (AppleScript never sees them as script text, only as `argv` it reads back).
fn osascript_args(title: &str, body: &str) -> Vec<String> {
    vec!["-e".to_string(), SCRIPT.to_string(), title.to_string(), body.to_string()]
}

fn send(title: &str, body: &str) -> Result<(), String> {
    match Command::new("osascript").args(osascript_args(title, body)).status() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("osascript exited with {s}")),
        Err(e) => Err(format!("osascript failed to start: {e}")),
    }
}

/// What "the same finding" means for dedupe: a group and the rule that fired, not the detail text,
/// which can change between polls (a host list growing, say) without being a new finding. Collides
/// only if a group_key itself ever contained a literal `:rule-name` suffix matching a real rule,
/// which group_key never does (it is always a filesystem path or an `apple:`/`dev:`/`other:` key).
fn finding_id(f: &Finding) -> String {
    format!("{}:{}", f.group_key, f.rule)
}

// SAFETY: tm is a plain value type, valid zeroed; localtime_r writes into it and never reads
// from it first, so an all-zero starting value cannot be observed as a real time.
fn local_minutes_since_midnight(unix_ts: u64) -> u16 {
    unsafe {
        let t = unix_ts as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        (tm.tm_hour * 60 + tm.tm_min) as u16
    }
}

/// The testable core of `dispatch`: takes the history handle, settings and current time as plain
/// values and `sender` as an injected function, so seeding, quiet hours, dedupe and the osascript
/// failure path can all be exercised without touching real files or firing a real OS notification.
/// Returns `Some(settings)` only when something changed (seeding just happened) and needs saving.
fn dispatch_with(h: &History, mut cfg: Settings, findings: &[Finding], now: u64, now_minutes: u16, sender: impl Fn(&str, &str) -> Result<(), String>) -> Option<Settings> {
    if cfg.notifications == Level::Off {
        return None;
    }

    if !cfg.seeded {
        for f in findings {
            let _ = h.mark_notified(&finding_id(f), now);
        }
        cfg.seeded = true;
        return Some(cfg);
    }

    if cfg.in_quiet_hours(now_minutes) {
        return None;
    }

    for f in findings {
        if cfg.notifications == Level::High && f.severity != Severity::High {
            continue;
        }
        let id = finding_id(f);
        match h.notified_recently(&id, now, DEDUPE_SECS) {
            Ok(true) => continue,
            Ok(false) => {}
            Err(_) => continue,
        }
        let (title, body) = if cfg.generic_text {
            ("Monitor Lizard".to_string(), "A finding needs your attention. Open the dashboard for details.".to_string())
        } else {
            (f.group.clone(), f.detail.clone())
        };
        match sender(&title, &body) {
            Ok(()) => {
                let _ = h.mark_notified(&id, now);
            }
            Err(e) => eprintln!("monitor-lizard: notification failed: {e}"),
        }
    }
    None
}

/// Sends at most one notification per finding per 24h, gated by the `notifications` setting and
/// quiet hours. The first time notifications are turned on from off, every finding present at
/// that moment is seeded as already-seen instead of notified, so enabling the feature never fires
/// a backlog of everything already on the machine.
pub fn dispatch(findings: &[Finding]) {
    let cfg = settings::load();
    if cfg.notifications == Level::Off {
        return;
    }
    let Some(h) = crate::history::open_or_report() else { return };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if let Some(updated) = dispatch_with(&h, cfg, findings, now, local_minutes_since_midnight(now), send) {
        let _ = settings::save(&updated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(group_key: &str, rule: &'static str, severity: Severity) -> Finding {
        Finding { severity, group: group_key.into(), group_key: group_key.into(), rule, detail: "detail".into() }
    }

    #[test]
    fn finding_id_combines_group_and_rule_not_detail() {
        let a = finding("k", "rule", Severity::High);
        let mut b = finding("k", "rule", Severity::High);
        b.detail = "a completely different detail string".into();
        assert_eq!(finding_id(&a), finding_id(&b), "the same group+rule is the same finding even if detail text changes between polls");
    }

    #[test]
    fn untrusted_title_and_body_are_passed_as_literal_argv_elements_not_script_text() {
        // The exact case from the todo's edge-case table: a name crafted to look like it is
        // closing the AppleScript string and starting a shell command.
        let evil = "\"; do shell script \"touch /tmp/pwned\"";
        let args = osascript_args(evil, "body with a \\ backslash and \" quote");
        assert_eq!(args[0], "-e");
        assert_eq!(args[1], SCRIPT, "the script text itself never contains the untrusted strings");
        assert_eq!(args[2], evil, "the title survives as one argv element, unescaped and unmodified");
        assert_eq!(args[3], "body with a \\ backslash and \" quote");
        assert_eq!(args.len(), 4, "exactly argv, nothing concatenated into a single string the shell could re-split");
    }

    fn tmp_history(name: &str) -> History {
        let dir = std::env::temp_dir().join(format!("monlizard-notify-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        History::at(&dir.join("h.db")).unwrap()
    }
    fn settings(notifications: Level, seeded: bool) -> Settings {
        Settings { notifications, seeded, ..Settings::default() }
    }
    fn calls(log: &std::sync::Mutex<Vec<(String, String)>>) -> impl Fn(&str, &str) -> Result<(), String> + '_ {
        move |t: &str, b: &str| { log.lock().unwrap().push((t.to_string(), b.to_string())); Ok(()) }
    }

    #[test]
    fn notifications_off_sends_nothing_and_does_not_seed() {
        let h = tmp_history("off");
        let log = std::sync::Mutex::new(vec![]);
        let out = dispatch_with(&h, settings(Level::Off, false), &[finding("k", "exposed-listener", Severity::High)], 1000, 0, calls(&log));
        assert!(out.is_none(), "nothing changed, nothing to save");
        assert!(log.lock().unwrap().is_empty());
        assert!(!h.notified_recently("k:exposed-listener", 1000, 86400).unwrap(), "off means not even seeded");
    }

    #[test]
    fn first_enable_seeds_every_finding_and_sends_nothing() {
        let h = tmp_history("seed");
        let log = std::sync::Mutex::new(vec![]);
        let findings = vec![finding("a", "exposed-listener", Severity::High), finding("b", "unreviewed", Severity::Medium)];
        let out = dispatch_with(&h, settings(Level::High, false), &findings, 1000, 0, calls(&log));
        assert!(log.lock().unwrap().is_empty(), "none sent, matching the edge case exactly");
        assert!(out.unwrap().seeded, "seeded is now true, so the caller knows to persist it");
        assert!(h.notified_recently("a:exposed-listener", 1000, 86400).unwrap(), "seeded as already-seen");
        assert!(h.notified_recently("b:unreviewed", 1000, 86400).unwrap(), "every finding is seeded, not only ones the current level would notify for");
    }

    #[test]
    fn quiet_hours_sends_nothing_and_does_not_mark_notified() {
        let h = tmp_history("quiet");
        let log = std::sync::Mutex::new(vec![]);
        let mut cfg = settings(Level::All, true);
        cfg.quiet_start = Some(0);
        cfg.quiet_end = Some(1439); // in_quiet_hours(700) is true for any start < 700 < end
        let out = dispatch_with(&h, cfg, &[finding("k", "exposed-listener", Severity::High)], 1000, 700, calls(&log));
        assert!(log.lock().unwrap().is_empty());
        assert!(out.is_none());
        assert!(!h.notified_recently("k:exposed-listener", 1000, 86400).unwrap(), "not marked, so it still notifies once quiet hours end");
    }

    #[test]
    fn a_never_notified_finding_sends_once_and_is_then_marked() {
        let h = tmp_history("send-once");
        let log = std::sync::Mutex::new(vec![]);
        dispatch_with(&h, settings(Level::High, true), &[finding("k", "exposed-listener", Severity::High)], 1000, 700, calls(&log));
        assert_eq!(log.lock().unwrap().len(), 1);
        assert!(h.notified_recently("k:exposed-listener", 1000, 86400).unwrap());
    }

    #[test]
    fn the_same_finding_on_the_next_100_polls_sends_only_once() {
        let h = tmp_history("hundred-polls");
        let log = std::sync::Mutex::new(vec![]);
        let f = [finding("k", "exposed-listener", Severity::High)];
        for i in 0..100u64 {
            dispatch_with(&h, settings(Level::High, true), &f, 1000 + i, 700, calls(&log));
        }
        assert_eq!(log.lock().unwrap().len(), 1, "one send on the first poll; the dedupe window covers the rest");
    }

    #[test]
    fn the_same_finding_25_hours_later_sends_again() {
        let h = tmp_history("25h");
        let log = std::sync::Mutex::new(vec![]);
        let f = [finding("k", "exposed-listener", Severity::High)];
        dispatch_with(&h, settings(Level::High, true), &f, 1000, 700, calls(&log));
        dispatch_with(&h, settings(Level::High, true), &f, 1000 + 25 * 3600, 700, calls(&log));
        assert_eq!(log.lock().unwrap().len(), 2);
    }

    #[test]
    fn high_only_ignores_medium_findings() {
        let h = tmp_history("high-only");
        let log = std::sync::Mutex::new(vec![]);
        dispatch_with(&h, settings(Level::High, true), &[finding("k", "unreviewed", Severity::Medium)], 1000, 700, calls(&log));
        assert!(log.lock().unwrap().is_empty());
    }

    #[test]
    fn generic_text_hides_the_real_group_and_detail() {
        let h = tmp_history("generic");
        let log = std::sync::Mutex::new(vec![]);
        let mut cfg = settings(Level::High, true);
        cfg.generic_text = false;
        let mut f = finding("k", "exposed-listener", Severity::High);
        f.group = "Real App Name".into();
        f.detail = "Real detail text".into();
        dispatch_with(&h, cfg, &[f], 1000, 700, calls(&log));
        assert_eq!(log.lock().unwrap()[0], ("Real App Name".to_string(), "Real detail text".to_string()));

        let h2 = tmp_history("generic2");
        let log2 = std::sync::Mutex::new(vec![]);
        let mut f2 = finding("k", "exposed-listener", Severity::High);
        f2.group = "Real App Name".into();
        dispatch_with(&h2, settings(Level::High, true), &[f2], 1000, 700, calls(&log2));
        let (title, body) = &log2.lock().unwrap()[0];
        assert!(!title.contains("Real App Name") && !body.contains("Real App Name"), "generic_text defaults true: no real app name leaks");
    }

    #[test]
    fn a_failed_send_is_not_marked_notified_so_it_retries() {
        let h = tmp_history("fail");
        let out = dispatch_with(&h, settings(Level::High, true), &[finding("k", "exposed-listener", Severity::High)], 1000, 700, |_, _| Err("permission denied".into()));
        assert!(out.is_none());
        assert!(!h.notified_recently("k:exposed-listener", 1000, 86400).unwrap(), "a failed send must not count as delivered");
    }

    #[test]
    fn minutes_since_midnight_is_in_range() {
        // Not asserting an exact value (depends on the machine's timezone and the current instant);
        // only that the conversion is sane, since this is a thin FFI wrapper with no branching to
        // unit test otherwise -- the real behaviour is exercised through Settings::in_quiet_hours.
        let m = local_minutes_since_midnight(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
        assert!(m < 1440);
    }
}
