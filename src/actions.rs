//! Process actions. Deliberately narrow: signal only your own processes, never launchd or ourselves.
use serde::Serialize;
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sig { Term, Kill }
impl Sig {
    fn flag(self) -> &'static str { match self { Sig::Term => "-TERM", Sig::Kill => "-KILL" } }
    pub fn name(self) -> &'static str { match self { Sig::Term => "TERM", Sig::Kill => "KILL" } }
}

/// `pids` and `start` are comma lists of equal length, paired by position (`pids[i]` started at
/// unix second `start[i]`): a pid is never accepted without the start time it was read at, so an
/// old client that only ever sent `pids` is refused outright rather than silently skipping the
/// recycling check `execute` now always makes.
pub fn parse(pids: &str, start: &str, sig: &str) -> Result<(Vec<(u32, u64)>, Sig), String> {
    let sig = match sig { "TERM" => Sig::Term, "KILL" => Sig::Kill, _ => return Err("sig must be TERM or KILL".into()) };
    let pids: Vec<u32> = pids.split(',').filter(|s| !s.is_empty())
        .map(|s| s.parse::<u32>().map_err(|_| format!("bad pid '{s}'"))).collect::<Result<Vec<_>, _>>()?;
    if pids.is_empty() || pids.len() > 200 { return Err("expected 1 to 200 pids".into()); }
    let starts: Vec<u64> = start.split(',').filter(|s| !s.is_empty())
        .map(|s| s.parse::<u64>().map_err(|_| format!("bad start time '{s}'"))).collect::<Result<Vec<_>, _>>()?;
    if starts.len() != pids.len() { return Err("expected one start time per pid".into()); }
    Ok((pids.into_iter().zip(starts).collect(), sig))
}

#[derive(Serialize, Debug)]
pub struct Refused { pub pid: u32, pub reason: String }
#[derive(Serialize, Debug, Default)]
pub struct Outcome { pub sent: Vec<u32>, pub refused: Vec<Refused> }

fn out(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

/// Whether `pid` belongs to `my_uid` at all, ignoring any specific instance's start time: a live
/// `ps` lookup, not a cached snapshot field, so it can never go stale. The one shared base both
/// `is_signalable` and `check` build on, so a pid refused here is refused everywhere.
fn basic_check(pid: u32, my_uid: &str) -> Result<(), &'static str> {
    if pid <= 1 { return Err("refusing to signal launchd"); }
    if pid == std::process::id() { return Err("refusing to signal monitor-lizard itself"); }
    let uid = out("ps", &["-o", "uid=", "-p", &pid.to_string()]);
    if uid.is_empty() { Err("no such process") } else if uid != my_uid { Err("not your process") } else { Ok(()) }
}

/// Whether a pid could ever be signalled right now, ignoring any specific instance's start time:
/// `classify::allowed_actions` uses this alone to decide which buttons to offer, and never signals
/// anything itself, so it has no particular instance's start time to defend.
pub(crate) fn is_signalable(pid: u32, my_uid: &str) -> bool { basic_check(pid, my_uid).is_ok() }

/// The real signal-time check: `basic_check` plus a fresh re-read of `pid`'s start time, refused as
/// changed if it no longer matches `expected_start` -- the recycled-pid defense. Used only where a
/// signal is about to actually be sent (`execute`), never for a plain "can this be signalled" query.
pub(crate) fn check(pid: u32, my_uid: &str, expected_start: u64) -> Result<(), &'static str> {
    basic_check(pid, my_uid)?;
    match crate::procusage::live_start_unix(pid) {
        Some(actual) if actual == expected_start => Ok(()),
        // Either genuinely a different process now (the old one exited and the pid was recycled),
        // or its start time could not be read fresh right now -- both get the same answer, since
        // only a confirmed match is ever safe to sign.
        _ => Err("process changed"),
    }
}

pub(crate) fn my_uid() -> String { out("id", &["-u"]) }
pub(crate) fn my_username() -> String { out("id", &["-un"]) }

pub fn execute(items: &[(u32, u64)], sig: Sig) -> Outcome {
    let me = my_uid();
    let mut o = Outcome::default();
    for &(pid, start) in items {
        match check(pid, &me, start) {
            Err(reason) => o.refused.push(Refused { pid, reason: reason.into() }),
            Ok(()) => match Command::new("kill").args([sig.flag(), &pid.to_string()]).status() {
                Ok(s) if s.success() => o.sent.push(pid),
                _ => o.refused.push(Refused { pid, reason: "kill failed".into() }),
            },
        }
    }
    o
}

/// Moves one path to the user's Trash through `NSFileManager`. The Finder route keeps "Put Back" but
/// asks for an Automation permission the first time, so it is not used. The caller has already
/// checked the path; this only does the move and returns the OS's reason when it fails.
pub fn trash(path: &std::path::Path) -> Result<(), String> {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    let mut ctx = trash::TrashContext::default();
    ctx.set_delete_method(DeleteMethod::NsFileManager);
    ctx.delete(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_requests() {
        assert_eq!(parse("12,34", "100,200", "TERM").unwrap(), (vec![(12, 100), (34, 200)], Sig::Term));
        assert!(parse("12", "100", "HUP").is_err());
        assert!(parse("", "", "TERM").is_err());
        assert!(parse("1;rm", "100", "KILL").is_err());
    }

    #[test]
    fn start_omitted_or_mismatched_in_count_is_refused_before_any_check_runs() {
        assert!(parse("12", "", "TERM").is_err(), "an old client that never sends start must be refused, not silently allowed");
        assert!(parse("12,34", "100", "TERM").is_err(), "one start time for two pids is not a per-pid pairing");
        assert!(parse("12", "100,200", "TERM").is_err());
    }

    #[test]
    fn refuses_launchd_and_self() {
        let o = execute(&[(1, 0), (std::process::id(), 0)], Sig::Term);
        assert!(o.sent.is_empty() && o.refused.len() == 2);
    }
    /// Touches the real Trash, so it only runs on request: `cargo test --lib real_trash -- --ignored`.
    #[test]
    #[ignore = "puts a file in the real Trash"]
    fn real_trash_takes_a_file_and_refuses_a_missing_one() {
        let p = std::env::temp_dir().join(format!("ml-real-trash-{}.txt", std::process::id()));
        std::fs::write(&p, "monitor-lizard trash test").unwrap();
        trash(&p).unwrap();
        assert!(!p.exists());
        assert!(trash(&p).is_err());
    }

    #[test]
    fn a_matching_start_time_passes_the_recheck_but_any_mismatch_is_refused_as_changed() {
        let mut child = Command::new("/bin/sh").args(["-c", "sleep 2"]).spawn().expect("spawn a real child to check against");
        let pid = child.id();
        // Give proc_pid_rusage a moment to have something to report for the new pid.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let real_start = crate::procusage::live_start_unix(pid).expect("read the child's own start time");
        let me = my_uid();
        assert_eq!(check(pid, &me, real_start), Ok(()));
        assert_eq!(check(pid, &me, real_start + 1), Err("process changed"), "a pid match with the wrong start time must never pass");
        assert!(is_signalable(pid, &me), "classify's signalability query has no specific start time to defend and must still say yes");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn a_pid_nothing_is_using_is_refused_even_with_the_right_looking_start_time() {
        assert!(check(999_999, &my_uid(), 123).is_err());
        assert!(!is_signalable(999_999, &my_uid()));
    }
}
