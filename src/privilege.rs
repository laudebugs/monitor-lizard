use crate::watcher::{Hop, SudoEvent};
use serde::Serialize;
use std::process::Command;

/// Where an authorization's requester chain came from.
#[derive(Serialize, Debug, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// A live-capture row for the same process: the full chain.
    Captured,
    /// `ps` found the process still running: its parent only.
    Ps,
    /// Neither: the process was gone and Monitor Lizard had not recorded it.
    None,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct PrivEvent {
    pub time: String, pub right: String, pub client: String, pub creator: String, pub pid: u32,
    pub ok: bool, pub error: Option<String>,
    /// requester's parent, only resolvable while the process is still alive
    pub parent: Option<String>,
    /// `time` as seconds since the epoch; None when the log's time stamp could not be read
    pub epoch: Option<u64>,
    pub source: Source,
    /// the live-capture chain when `source` is `captured`, nearest parent first
    pub chain: Vec<Hop>,
}

/// What the page and the CLI say when no chain could be found for an authorization.
pub const UNKNOWN_REQUESTER: &str = "launched before Monitor Lizard was running or exited before capture";

/// authd's `2026-11-26 14:03:22.123456-0800` as seconds since the epoch.
pub fn parse_log_time(t: &str) -> Option<u64> {
    let (date, rest) = t.split_once(' ')?;
    let mut d = date.split('-');
    let (y, m, day): (i64, i64, i64) = (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
    if d.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&day) { return None; }
    let sign_at = rest.rfind(['+', '-'])?;
    let (clock, zone) = rest.split_at(sign_at);
    let mut c = clock.split(':');
    let (hh, mm): (i64, i64) = (c.next()?.parse().ok()?, c.next()?.parse().ok()?);
    let ss: i64 = c.next()?.split('.').next()?.parse().ok()?;
    if hh > 23 || mm > 59 || ss > 60 || zone.len() != 5 { return None; }
    let off = zone[1..3].parse::<i64>().ok()? * 3600 + zone[3..5].parse::<i64>().ok()? * 60;
    let off = if zone.starts_with('-') { -off } else { off };
    // days since 1970-01-01 (Howard Hinnant's civil-from-days, inverted)
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + day - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss - off).ok()
}

/// A live capture is taken a moment after the process appears, so its time can sit a little after the log line.
const LEAD_SLACK: u64 = 10;
/// sudo waits for a password up to five minutes, and the failure is logged when that ends.
const WINDOW: u64 = 600;

/// The live-capture row that explains an authorization by `client` pid `pid` logged at `epoch`: same pid, same
/// program, captured no more than `LEAD_SLACK` seconds after and no more than `WINDOW` seconds before the log
/// line. When a pid was reused inside the window, the most recent capture before the line wins.
pub fn match_live<'a>(epoch: u64, pid: u32, client: &str, live: &'a [SudoEvent]) -> Option<&'a SudoEvent> {
    live.iter()
        .filter(|e| e.pid == pid && e.tool == client && e.ts <= epoch + LEAD_SLACK && epoch <= e.ts + WINDOW)
        .max_by_key(|e| (e.ts <= epoch, e.ts.min(epoch + LEAD_SLACK)))
}

/// Gives each event its requester: the matched live chain, else `lookup`'s answer for a process that is still
/// running, else nothing. `lookup` is `ps` in production and a stub in tests.
pub fn resolve(evs: &mut [PrivEvent], live: &[SudoEvent], lookup: impl Fn(u32) -> Option<String>) {
    for e in evs {
        if let Some(m) = e.epoch.and_then(|t| match_live(t, e.pid, &e.client, live)) {
            e.chain = m.chain.clone();
            e.source = Source::Captured;
            e.parent = None;
        } else if let Some(p) = lookup(e.pid) {
            e.parent = Some(p);
            e.source = Source::Ps;
        }
    }
}

fn between<'a>(s: &'a str, a: &str, b: &str) -> Option<&'a str> {
    let i = s.find(a)? + a.len();
    let j = s[i..].find(b)? + i;
    Some(&s[i..j])
}

/// Parses authd messages such as
/// "Failed to authorize right 'R' by client 'C' [123] for authorization created by 'X' [123] (3,0) (-60006)".
pub fn parse_message(time: &str, msg: &str) -> Option<PrivEvent> {
    let ok = msg.contains("Succeeded authorizing right");
    if !ok && !msg.contains("Failed to authorize right") { return None; }
    let right = between(msg, "right '", "'")?.to_string();
    let client = between(msg, "by client '", "'")?.to_string();
    let after = &msg[msg.find("by client '")?..];
    let pid = between(after, "' [", "]")?.parse().ok()?;
    let creator = between(msg, "created by '", "'")?.to_string();
    let error = (!ok).then(|| between(msg, ") (", ")").unwrap_or("failed").to_string());
    Some(PrivEvent { time: time.into(), right, client, creator, pid, ok, error, parent: None, epoch: parse_log_time(time), source: Source::None, chain: vec![] })
}

fn parent_of(pid: u32) -> Option<String> {
    let out = Command::new("ps").args(["-o", "ppid=,command=", "-p", &pid.to_string()]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Authorization requests (sudo, admin prompts, system settings unlocks) from the last `minutes`, each joined to
/// the live-capture row for the same process when there is one (`live`, newest first or not: order is not used).
pub fn recent(minutes: u32, live: &[SudoEvent]) -> Vec<PrivEvent> {
    let out = Command::new("/usr/bin/log")
        .args(["show", "--last", &format!("{minutes}m"), "--info", "--style", "ndjson", "--predicate",
               "process == \"authd\" AND (eventMessage CONTAINS \"authorize right\" OR eventMessage CONTAINS \"authorizing right\")"])
        .output();
    let Ok(out) = out else { return vec![] };
    let mut evs: Vec<PrivEvent> = String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| {
        let v: serde_json::Value = serde_json::from_str(l).ok()?;
        parse_message(v["timestamp"].as_str()?, v["eventMessage"].as_str()?)
    }).collect();
    resolve(&mut evs, live, parent_of);
    evs.reverse(); // newest first
    evs
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_failed_sudo() {
        let m = "Failed to authorize right 'com.apple.security.sudo' by client '/usr/bin/sudo' [66003] for authorization created by '/usr/bin/sudo' [66003] (3,0) (-60006) (engine 2418)";
        let e = parse_message("t", m).unwrap();
        assert_eq!((e.right.as_str(), e.pid, e.ok), ("com.apple.security.sudo", 66003, false));
        assert_eq!(e.client, "/usr/bin/sudo");
        assert_eq!(e.error.as_deref(), Some("-60006"));
    }
    fn live(ts: u64, pid: u32, tool: &str) -> SudoEvent {
        SudoEvent { ts, pid, tool: tool.into(), args: "sudo x".into(), chain: vec![Hop { pid: 1000 + pid, command: "bash".into(), ..Hop::default() }] }
    }
    const SUDO: &str = "/usr/bin/sudo";

    #[test]
    fn log_times_become_epoch_seconds() {
        assert_eq!(parse_log_time("1970-01-01 00:00:00.000000+0000"), Some(0));
        assert_eq!(parse_log_time("2026-11-26 14:03:22.123456-0800"), Some(1_795_730_602));
        assert_eq!(parse_log_time("2024-02-29 23:59:59.5+0130"), Some(1_709_245_799));
        assert_eq!(parse_log_time("2000-03-01 00:00:00+0000"), Some(951_868_800));
    }
    #[test]
    fn unreadable_log_times_are_none() {
        for t in ["", "yesterday", "2026-13-01 00:00:00+0000", "2026-11-26 25:00:00+0000", "2026-11-26 14:03:22", "2026-11-26 14:03:22+08"] {
            assert_eq!(parse_log_time(t), None, "{t}");
        }
    }
    #[test]
    fn matches_the_same_pid_within_the_window() {
        let l = [live(1000, 66003, SUDO)];
        assert!(match_live(1000, 66003, SUDO, &l).is_some());
        assert!(match_live(1000 + 599, 66003, SUDO, &l).is_some(), "a long password prompt");
        assert!(match_live(995, 66003, SUDO, &l).is_some(), "capture a little after the log line");
    }
    #[test]
    fn does_not_match_outside_the_window_another_pid_or_another_program() {
        let l = [live(1000, 66003, SUDO)];
        assert!(match_live(1000 + 601, 66003, SUDO, &l).is_none(), "pid reused much later");
        assert!(match_live(980, 66003, SUDO, &l).is_none(), "capture long after the log line");
        assert!(match_live(1000, 66004, SUDO, &l).is_none());
        assert!(match_live(1000, 66003, "/usr/bin/su", &l).is_none());
        assert!(match_live(1000, 66003, SUDO, &[]).is_none());
    }
    #[test]
    fn a_reused_pid_picks_the_nearest_earlier_capture() {
        let l = [live(100, 5, SUDO), live(400, 5, SUDO), live(900, 5, SUDO)];
        assert_eq!(match_live(450, 5, SUDO, &l).unwrap().ts, 400);
        assert_eq!(match_live(905, 5, SUDO, &l).unwrap().ts, 900);
    }
    fn failed(pid: u32, time: &str) -> PrivEvent {
        parse_message(time, &format!("Failed to authorize right 'com.apple.security.sudo' by client '{SUDO}' [{pid}] for authorization created by '{SUDO}' [{pid}] (3,0) (-60006)")).unwrap()
    }
    #[test]
    fn resolve_prefers_the_live_chain_then_ps_then_says_nothing() {
        let t = "2026-11-26 14:03:22.000000-0800";
        let at = parse_log_time(t).unwrap();
        let mut evs = vec![failed(1, t), failed(2, t), failed(3, t)];
        resolve(&mut evs, &[live(at - 3, 1, SUDO)], |pid| (pid == 2).then(|| "77 bash -c x".to_string()));
        assert_eq!((evs[0].source, evs[0].chain.len(), evs[0].parent.clone()), (Source::Captured, 1, None));
        assert_eq!((evs[1].source, evs[1].parent.as_deref()), (Source::Ps, Some("77 bash -c x")));
        assert_eq!((evs[2].source, evs[2].parent.clone(), evs[2].chain.len()), (Source::None, None, 0));
    }
    #[test]
    fn an_event_with_an_unreadable_time_is_never_matched() {
        let mut evs = vec![failed(1, "garbled")];
        resolve(&mut evs, &[live(0, 1, SUDO)], |_| None);
        assert_eq!(evs[0].source, Source::None);
    }
    #[test]
    fn ignores_noise() { assert!(parse_message("t", "copy_rights: authorization failed").is_none()); }
}
