//! What describes the boot volume rather than a folder: capacity (with the purgeable split) and
//! local Time Machine snapshots.
use super::reason;
use crate::sampler::Reading;
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use serde::Serialize;
use std::io::{self, Read};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const AVAILABLE_KEY: &str = "NSURLVolumeAvailableCapacityKey";
const IMPORTANT_KEY: &str = "NSURLVolumeAvailableCapacityForImportantUsageKey";
pub const TMUTIL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Serialize, Debug)]
pub struct Capacity { pub total: Reading<u64>, pub used: Reading<u64>, pub purgeable: Reading<u64>, pub free: Reading<u64> }

#[derive(Serialize, Debug)]
pub struct Snapshots { pub count: Reading<usize>, pub dates: Vec<String>, pub size: Reading<u64> }

/// `statfs` gives total and free (what `df` shows); the "important usage" figure adds space macOS
/// would reclaim on demand. Their difference is purgeable. `statfs` is (total, used including
/// purgeable) and `volume` is (available, important).
pub fn split_capacity(statfs: Option<(u64, u64)>, volume: Option<(u64, u64)>) -> Capacity {
    let Some((total, used_with_purgeable)) = statfs else {
        let gone = || Reading::Unavailable(reason::STATFS_FAILED);
        return Capacity { total: gone(), used: gone(), purgeable: gone(), free: gone() };
    };
    let free = total.saturating_sub(used_with_purgeable);
    match volume {
        Some((available, important)) => {
            let purgeable = important.saturating_sub(available).min(used_with_purgeable);
            Capacity { total: Reading::Value(total), used: Reading::Value(used_with_purgeable - purgeable), purgeable: Reading::Value(purgeable), free: Reading::Value(free) }
        }
        // Without the purgeable split, `used` still counts it.
        None => Capacity { total: Reading::Value(total), used: Reading::Value(used_with_purgeable), purgeable: Reading::Unavailable(reason::VOLUME_KEYS_UNREADABLE), free: Reading::Value(free) },
    }
}

/// (available, important-usage) bytes for `/`, readable without root, through the safe
/// CoreFoundation wrappers: every object is released by its wrapper, so nothing is hand-released.
pub fn available_and_important() -> Option<(u64, u64)> {
    let url = CFURL::from_path("/", true)?;
    let keys = CFArray::from_CFTypes(&[CFString::new(AVAILABLE_KEY), CFString::new(IMPORTANT_KEY)]);
    // SAFETY: the call follows the Create rule, so the returned dictionary is owned by the wrapper,
    // which releases it; the URL and key array outlive the call; a null result is checked first.
    let dict = unsafe {
        let raw = core_foundation_sys::url::CFURLCopyResourcePropertiesForKeys(url.as_concrete_TypeRef(), keys.as_concrete_TypeRef(), std::ptr::null_mut());
        if raw.is_null() { return None; }
        CFDictionary::<CFString, CFType>::wrap_under_create_rule(raw)
    };
    let number = |key: &str| dict.find(&CFString::new(key))?.downcast::<CFNumber>()?.to_i64().and_then(|n| u64::try_from(n).ok());
    Some((number(AVAILABLE_KEY)?, number(IMPORTANT_KEY)?))
}

/// `tmutil` output -> snapshot dates (`YYYY-MM-DD-HHMMSS`). OS-update snapshots are not Time Machine's and are skipped.
pub fn parse_snapshots(out: &str) -> Vec<String> {
    out.lines().filter_map(|l| l.trim().strip_prefix("com.apple.TimeMachine.")?.strip_suffix(".local").map(str::to_string)).collect()
}

/// Runs a command with a time limit and returns its output. The child is killed on timeout.
pub fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<String, &'static str> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        .map_err(|e| if e.kind() == io::ErrorKind::NotFound { reason::TMUTIL_NOT_FOUND } else { reason::TMUTIL_COULD_NOT_START })?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(25)),
            _ => { let _ = child.kill(); let _ = child.wait(); return Err(reason::TMUTIL_TIMED_OUT); }
        }
    };
    if !status.success() { return Err(reason::TMUTIL_FAILED); }
    let mut text = String::new();
    child.stdout.take().ok_or(reason::TMUTIL_FAILED)?.read_to_string(&mut text).map_err(|_| reason::TMUTIL_FAILED)?;
    Ok(text)
}

pub fn snapshots_from(out: Result<String, &'static str>) -> Snapshots {
    match out {
        Ok(text) => {
            let dates = parse_snapshots(&text);
            // As of 2026-11, neither `tmutil` nor `diskutil apfs listSnapshots` reports a size.
            Snapshots { count: Reading::Value(dates.len()), dates, size: Reading::Unavailable(reason::SNAPSHOT_SIZE) }
        }
        Err(why) => Snapshots { count: Reading::Unavailable(why), dates: Vec::new(), size: Reading::Unavailable(why) },
    }
}

/// Local snapshots of `/`, tolerating `tmutil` missing, failing or hanging.
pub fn local_snapshots() -> Snapshots {
    snapshots_from(run_with_timeout(Command::new("/usr/bin/tmutil").args(["listlocalsnapshots", "/"]), TMUTIL_TIMEOUT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_splits_purgeable_from_free_and_says_why_when_it_cannot() {
        let c = split_capacity(Some((1000, 600)), Some((300, 450)));
        assert_eq!((c.total, c.free, c.purgeable, c.used), (Reading::Value(1000), Reading::Value(400), Reading::Value(150), Reading::Value(450)));
        let c = split_capacity(Some((1000, 600)), None);
        assert_eq!((c.free, c.used), (Reading::Value(400), Reading::Value(600)));
        assert_eq!(c.purgeable, Reading::Unavailable(reason::VOLUME_KEYS_UNREADABLE));
        // An estimate that exceeds what is used is clamped, never negative.
        let c = split_capacity(Some((1000, 100)), Some((900, 5000)));
        assert_eq!(c.purgeable, Reading::Value(100));
        assert_eq!(split_capacity(None, None).total, Reading::Unavailable(reason::STATFS_FAILED));
    }

    #[test]
    fn the_real_volume_is_readable_without_root_and_repeated_reads_agree() {
        let first = available_and_important().expect("readable without root");
        assert!(first.0 > 0 && first.1 >= first.0 / 2, "{first:?}");
        for _ in 0..200 {
            let again = available_and_important().unwrap();
            // Free space moves a little between reads, never by half.
            assert!(again.0 / 2 < first.0 && first.0 < again.0 * 2);
        }
    }

    #[test]
    fn snapshots_parse_dates_and_say_why_when_tmutil_fails() {
        let out = "Snapshots for disk /:\ncom.apple.TimeMachine.2026-11-25-101500.local\ncom.apple.os.update-ABC\ncom.apple.TimeMachine.2026-11-26-091500.local\n";
        let s = snapshots_from(Ok(out.into()));
        assert_eq!((s.count, s.dates), (Reading::Value(2), vec!["2026-11-25-101500".to_string(), "2026-11-26-091500".to_string()]));
        assert_eq!(s.size, Reading::Unavailable(reason::SNAPSHOT_SIZE));
        let s = snapshots_from(Err(reason::TMUTIL_NOT_FOUND));
        assert_eq!((s.count, s.size), (Reading::Unavailable(reason::TMUTIL_NOT_FOUND), Reading::Unavailable(reason::TMUTIL_NOT_FOUND)));
        assert_eq!(snapshots_from(Ok(String::new())).count, Reading::Value(0));
    }

    #[test]
    fn a_command_that_hangs_is_killed_and_reported() {
        let t = Instant::now();
        let r = run_with_timeout(Command::new("/bin/sleep").arg("30"), Duration::from_millis(150));
        assert_eq!(r, Err(reason::TMUTIL_TIMED_OUT));
        assert!(t.elapsed() < Duration::from_secs(10), "the child was not left running to completion");
    }

    #[test]
    fn a_missing_or_failing_command_is_reported_not_hidden() {
        assert_eq!(run_with_timeout(&mut Command::new("/nonexistent/tmutil"), Duration::from_secs(1)), Err(reason::TMUTIL_NOT_FOUND));
        assert_eq!(run_with_timeout(&mut Command::new("/usr/bin/false"), Duration::from_secs(1)), Err(reason::TMUTIL_FAILED));
        assert_eq!(snapshots_from(run_with_timeout(&mut Command::new("/usr/bin/false"), Duration::from_secs(1))).count, Reading::Unavailable(reason::TMUTIL_FAILED));
    }
}
