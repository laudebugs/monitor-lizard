//! Native macOS notifications for high-severity findings, off by default. `osascript` is invoked
//! with `on run argv` so the title and body are always passed as process arguments, never built
//! into the script text: the only safe way to hand it untrusted strings (a process name, a path).
use crate::{
    history::History,
    model::{Finding, Severity},
    settings::{self, Level},
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

/// Sends at most one notification per finding per 24h, gated by the `notifications` setting and
/// quiet hours. The first time notifications are turned on from off, every finding present at
/// that moment is seeded as already-seen instead of notified, so enabling the feature never fires
/// a backlog of everything already on the machine.
pub fn dispatch(findings: &[Finding]) {
    let cfg = settings::load();
    if cfg.notifications == Level::Off {
        return;
    }
    let Ok(h) = History::open() else { return };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

    if !cfg.seeded {
        for f in findings {
            let _ = h.mark_notified(&finding_id(f), now);
        }
        let mut cfg = cfg;
        cfg.seeded = true;
        let _ = settings::save(&cfg);
        return;
    }

    if cfg.in_quiet_hours(local_minutes_since_midnight(now)) {
        return;
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
            ("Sys Monitor".to_string(), "A finding needs your attention. Open the dashboard for details.".to_string())
        } else {
            (f.group.clone(), f.detail.clone())
        };
        match send(&title, &body) {
            Ok(()) => {
                let _ = h.mark_notified(&id, now);
            }
            Err(e) => eprintln!("sys-monitor: notification failed: {e}"),
        }
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

    #[test]
    fn minutes_since_midnight_is_in_range() {
        // Not asserting an exact value (depends on the machine's timezone and the current instant);
        // only that the conversion is sane, since this is a thin FFI wrapper with no branching to
        // unit test otherwise -- the real behaviour is exercised through Settings::in_quiet_hours.
        let m = local_minutes_since_midnight(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
        assert!(m < 1440);
    }
}
