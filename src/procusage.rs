//! Per-process CPU, memory footprint and disk I/O for your own processes, read every 5 s via
//! `proc_pid_rusage` and rate-computed from diffs keyed by (pid, start time) so a recycled pid is
//! never diffed against whatever used to have it. Another user's process can't be read this way
//! (a permission error); CPU and memory fall back to `ps`'s own columns, already read by
//! `collect::processes`, and disk is reported as unavailable rather than guessed.
use crate::model::Proc;
use crate::sampler::Reading;
use libproc::libproc::pid_rusage::{pidrusage, RUsageInfoV0, RUsageInfoV4};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct ProcessUsage {
    pub pid: u32,
    pub command: String,
    pub restricted: bool,
    pub cpu_pct: Reading<f32>,
    pub mem_bytes: Reading<u64>,
    pub disk_read_bytes_per_sec: Option<Reading<f64>>,
    pub disk_write_bytes_per_sec: Option<Reading<f64>>,
    /// Unix seconds the process started, for the Stop confirm dialog to echo back on `/api/kill` so
    /// a pid recycled since this row was read is refused rather than signalled (`actions::check`).
    pub start_unix: Reading<u64>,
}

/// What a real `proc_pid_rusage` read gives us, already converted to real units: mach
/// absolute-time ticks to nanoseconds (via `mach_timebase_info`), everything else is already
/// bytes. `disk` is `None` only when the flavor that succeeded (the V0 fallback) doesn't carry it.
struct RawUsage {
    start_abstime: u64,
    user_ns: u64,
    system_ns: u64,
    mem_bytes: u64,
    disk_read: Option<u64>,
    disk_write: Option<u64>,
}

// Unlike host_processor_info/host_statistics64 (used elsewhere in this codebase via libc, since
// mach2 doesn't expose those), mach2 does expose this one directly, so no deprecated libc path is
// needed here.
fn mach_timebase() -> (u32, u32) {
    // SAFETY: mach_timebase_info is a plain value type; an all-zero bit pattern is a valid value
    // for it (every field is a plain integer), and the struct is only read after the call below
    // has written into it.
    let mut info: mach2::mach_time::mach_timebase_info = unsafe { std::mem::zeroed() };
    // SAFETY: `&mut info` is a valid pointer to a correctly-sized buffer for this call to write
    // into; the call is documented to always succeed on a real Mac, so there is no failure mode
    // to check for beyond the numer/denom sanity fallback below.
    unsafe { mach2::mach_time::mach_timebase_info(&mut info) };
    if info.numer == 0 || info.denom == 0 { (1, 1) } else { (info.numer, info.denom) }
}

fn abstime_to_ns(ticks: u64, (numer, denom): (u32, u32)) -> u64 {
    // u128 so a long-running process's large tick count can't overflow the multiply.
    (ticks as u128 * numer as u128 / denom as u128) as u64
}

/// `kern.boottime`'s `tv_sec`: unix seconds this Mac last booted. `ri_proc_start_abstime` is itself
/// mach absolute time since boot, not since the epoch, so this is the other half needed to turn a
/// process's start time into something comparable across samples taken at different wall-clock
/// moments (a plain unix timestamp), rather than raw ticks whose meaning depends on this one read.
fn boottime_unix() -> Option<u64> {
    let name = std::ffi::CString::new("kern.boottime").ok()?;
    let mut tv: libc::timeval = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::timeval>();
    // SAFETY: `name` is a valid NUL-terminated sysctl name; `tv` is a plain value type (no
    // invariants an all-zero or partially-written bit pattern could violate) and is only read
    // after the call returns 0, the contract under which it has written a full `timeval`.
    let rc = unsafe { libc::sysctlbyname(name.as_ptr(), &mut tv as *mut _ as *mut libc::c_void, &mut len, std::ptr::null_mut(), 0) };
    (rc == 0 && tv.tv_sec >= 0).then_some(tv.tv_sec as u64)
}

fn start_unix(start_abstime: u64, timebase: (u32, u32), boottime: Option<u64>) -> Reading<u64> {
    match boottime {
        Some(boot) => Reading::Value(boot + abstime_to_ns(start_abstime, timebase) / 1_000_000_000),
        None => Reading::Unavailable("sysctl kern.boottime failed"),
    }
}

/// A genuinely fresh, uncached read of one pid's start time (unix seconds), for the moment of Stop:
/// the 5 s background cache (`Rows`) is fresh enough for the confirm dialog's own display, but the
/// decision to actually signal a process re-checks this directly, not whatever the cache last held.
/// `None` means the pid could not be read right now (gone, or owned by another user) -- `actions::check`
/// treats that the same as a pid that no longer matches: refused, never signalled.
pub fn live_start_unix(pid: u32) -> Option<u64> {
    let raw = read_rusage(pid as i32, mach_timebase()).ok()?;
    match start_unix(raw.start_abstime, mach_timebase(), boottime_unix()) {
        Reading::Value(v) => Some(v),
        _ => None,
    }
}

/// `proc_pid_rusage(pid, V4, ...)` first (has disk I/O); on failure that is not a permission error
/// (an unsupported flavor is rare but real on some sandboxed or older processes), falls back to V0
/// (always present, no disk fields) so the row still reports CPU and memory rather than nothing.
fn read_rusage(pid: i32, timebase: (u32, u32)) -> Result<RawUsage, String> {
    match pidrusage::<RUsageInfoV4>(pid) {
        Ok(r) => Ok(RawUsage {
            start_abstime: r.ri_proc_start_abstime,
            user_ns: abstime_to_ns(r.ri_user_time, timebase),
            system_ns: abstime_to_ns(r.ri_system_time, timebase),
            mem_bytes: r.ri_phys_footprint,
            disk_read: Some(r.ri_diskio_bytesread),
            disk_write: Some(r.ri_diskio_byteswritten),
        }),
        // `libproc` reports errors as free-form text, not a typed errno, so this is matched on
        // wording rather than a stable code -- a real fragility if that wording ever changes
        // upstream, worth revisiting if a future libproc version's error text diverges from this.
        Err(e) if e.contains("Permission") || e.contains("EPERM") || e.contains("EACCES") => Err(e),
        Err(_) => pidrusage::<RUsageInfoV0>(pid).map(|r| RawUsage {
            start_abstime: r.ri_proc_start_abstime,
            user_ns: abstime_to_ns(r.ri_user_time, timebase),
            system_ns: abstime_to_ns(r.ri_system_time, timebase),
            mem_bytes: r.ri_resident_size,
            disk_read: None,
            disk_write: None,
        }),
    }
}

struct Prev {
    start_abstime: u64,
    at: Instant,
    user_ns: u64,
    system_ns: u64,
    disk_read: Option<u64>,
    disk_write: Option<u64>,
}

pub struct Tracker {
    prev: HashMap<u32, Prev>,
    timebase: (u32, u32),
    /// Read once: the boot time does not change while this process runs.
    boottime: Option<u64>,
}

impl Default for Tracker {
    fn default() -> Self {
        Self { prev: HashMap::new(), timebase: mach_timebase(), boottime: boottime_unix() }
    }
}

impl Tracker {
    /// One row per process in `procs` (own-user first for a slightly more useful default order;
    /// the server sorts and limits per the request). Own-user processes that have exited between
    /// `collect::processes()` returning and this read are silently skipped (the edge case "Process
    /// exits mid-read -> Row skipped"), since `proc_pid_rusage` simply fails for a pid that no
    /// longer exists -- the same error path as any other read failure, not specially detected.
    pub fn sample(&mut self, procs: &[Proc], me: &str) -> Vec<ProcessUsage> {
        let now = Instant::now();
        let mut out = Vec::with_capacity(procs.len());
        let mut seen = std::collections::HashSet::new();
        for p in procs {
            seen.insert(p.pid);
            if p.user != me {
                out.push(ProcessUsage {
                    pid: p.pid, command: p.command.clone(), restricted: true,
                    cpu_pct: Reading::Value(p.cpu), mem_bytes: Reading::Value(p.rss_kb * 1024),
                    disk_read_bytes_per_sec: Some(Reading::Unavailable("another user's process")),
                    disk_write_bytes_per_sec: Some(Reading::Unavailable("another user's process")),
                    // Never signalled either way (not ours), so there is nothing to echo back on
                    // `/api/kill` for it; restricted rows get no Stop button in the first place.
                    start_unix: Reading::Unavailable("another user's process"),
                });
                continue;
            }
            let Ok(raw) = read_rusage(p.pid as i32, self.timebase) else {
                // Could not read rusage for our own process: it exited between the process list
                // being built and this read, or some other transient failure. Row skipped.
                self.prev.remove(&p.pid);
                continue;
            };
            let prev = prev_for(&self.prev, p.pid, raw.start_abstime);
            let usage = build_usage(p.pid, p.command.clone(), &raw, prev, now, self.timebase, self.boottime);
            self.prev.insert(p.pid, Prev {
                start_abstime: raw.start_abstime, at: now, user_ns: raw.user_ns, system_ns: raw.system_ns,
                disk_read: raw.disk_read, disk_write: raw.disk_write,
            });
            out.push(usage);
        }
        self.prev.retain(|pid, _| seen.contains(pid));
        out
    }
}

/// The one place "is this the same process as last time" is decided: a pid match alone is not
/// enough, since pids are recycled -- only a matching `start_abstime` too means it is genuinely
/// the same process, not a new one that happens to have reused the old pid. Pulled out of
/// `Tracker::sample` as its own named function (rather than an inline `.filter(...)` call) so a
/// test can exercise the real decision directly instead of a copy of its logic.
fn prev_for(cache: &HashMap<u32, Prev>, pid: u32, start_abstime: u64) -> Option<&Prev> {
    cache.get(&pid).filter(|pv| pv.start_abstime == start_abstime)
}

/// Pure: the rate math, separated from the `proc_pid_rusage` call itself so it can be tested
/// without a real process to read. `prev` being `None` (first sample, or `start_abstime` changed
/// because the pid was recycled) reports no rate, never a zero rate -- `cpu_pct`/`mem_bytes` are
/// never a rate, so they are always reported from `raw` directly.
fn build_usage(pid: u32, command: String, raw: &RawUsage, prev: Option<&Prev>, now: Instant, timebase: (u32, u32), boottime: Option<u64>) -> ProcessUsage {
    let start = start_unix(raw.start_abstime, timebase, boottime);
    let no_disk = || Some(Reading::Unavailable("disk I/O not available for this process (rusage fell back to an older version)"));
    let no_rate = |reason: &'static str| ProcessUsage {
        pid, command: command.clone(), restricted: false,
        cpu_pct: Reading::Unavailable(reason),
        mem_bytes: Reading::Value(raw.mem_bytes),
        disk_read_bytes_per_sec: if raw.disk_read.is_some() { None } else { no_disk() },
        disk_write_bytes_per_sec: if raw.disk_write.is_some() { None } else { no_disk() },
        start_unix: start.clone(),
    };
    let Some(prev) = prev else { return no_rate("not sampled yet") };
    let elapsed = now.duration_since(prev.at).as_secs_f64();
    if elapsed <= 0.0 {
        return no_rate("sampled again at the same instant as the last reading");
    }
    let cpu_ns = (raw.user_ns + raw.system_ns).saturating_sub(prev.user_ns + prev.system_ns);
    let cpu_pct = Reading::Value(((cpu_ns as f64 / 1e9) / elapsed * 100.0) as f32);
    let disk_rate = |now_v: Option<u64>, prev_v: Option<u64>| match (now_v, prev_v) {
        (Some(n), Some(p)) => Some(Reading::Value(n.saturating_sub(p) as f64 / elapsed)),
        _ => no_disk(),
    };
    ProcessUsage {
        pid, command, restricted: false, cpu_pct, mem_bytes: Reading::Value(raw.mem_bytes),
        disk_read_bytes_per_sec: disk_rate(raw.disk_read, prev.disk_read),
        disk_write_bytes_per_sec: disk_rate(raw.disk_write, prev.disk_write),
        start_unix: start,
    }
}

pub type Rows = Arc<RwLock<Vec<ProcessUsage>>>;

/// Spawns the 5 s background sampler and returns the shared, read-only row cache; `sort`/`limit`
/// are applied at request time in `src/server.rs`, not here.
pub fn start(me: String) -> Rows {
    let rows: Rows = Arc::new(RwLock::new(Vec::new()));
    let r = rows.clone();
    std::thread::spawn(move || {
        let mut tracker = Tracker::default();
        loop {
            let procs = crate::collect::processes();
            let sampled = tracker.sample(&procs, &me);
            if let Ok(mut w) = r.write() {
                *w = sampled;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    rows
}

pub fn sort_and_limit(mut rows: Vec<ProcessUsage>, sort: &str, limit: usize) -> Vec<ProcessUsage> {
    let key = |u: &ProcessUsage| -> f64 {
        match sort {
            "mem" => match u.mem_bytes { Reading::Value(v) => v as f64, _ => f64::MIN },
            "disk" => match (&u.disk_read_bytes_per_sec, &u.disk_write_bytes_per_sec) {
                (Some(Reading::Value(r)), Some(Reading::Value(w))) => r + w,
                (Some(Reading::Value(r)), _) => *r,
                (_, Some(Reading::Value(w))) => *w,
                _ => f64::MIN,
            },
            _ => match u.cpu_pct { Reading::Value(v) => v as f64, _ => f64::MIN }, // "cpu" and any unknown value default to cpu order
        }
    };
    rows.sort_by(|a, b| key(b).partial_cmp(&key(a)).unwrap_or(std::cmp::Ordering::Equal));
    rows.truncate(limit.max(1));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(start: u64, user_ns: u64, system_ns: u64, mem: u64, disk: Option<(u64, u64)>) -> RawUsage {
        RawUsage { start_abstime: start, user_ns, system_ns, mem_bytes: mem, disk_read: disk.map(|d| d.0), disk_write: disk.map(|d| d.1) }
    }
    fn prev(start: u64, at: Instant, user_ns: u64, system_ns: u64, disk: Option<(u64, u64)>) -> Prev {
        Prev { start_abstime: start, at, user_ns, system_ns, disk_read: disk.map(|d| d.0), disk_write: disk.map(|d| d.1) }
    }

    #[test]
    fn first_sample_has_no_cpu_rate_but_reports_memory() {
        let u = build_usage(100, "x".into(), &raw(1000, 50_000_000, 10_000_000, 2_000_000, Some((100, 50))), None, Instant::now(), (1, 1), Some(0));
        assert_eq!(u.cpu_pct, Reading::Unavailable("not sampled yet"));
        assert_eq!(u.mem_bytes, Reading::Value(2_000_000));
    }

    #[test]
    fn cpu_rate_from_two_samples() {
        let t0 = Instant::now();
        let p = prev(1000, t0, 0, 0, Some((0, 0)));
        // 1 second of real time later, 0.3s of combined user+system time used => 30%.
        let u = build_usage(100, "x".into(), &raw(1000, 200_000_000, 100_000_000, 1_000_000, Some((500, 200))), Some(&p), t0 + Duration::from_secs(1), (1, 1), Some(0));
        assert_eq!(u.cpu_pct, Reading::Value(30.0));
        assert_eq!(u.disk_read_bytes_per_sec, Some(Reading::Value(500.0)));
        assert_eq!(u.disk_write_bytes_per_sec, Some(Reading::Value(200.0)));
    }

    #[test]
    fn recycled_pid_is_a_new_process_not_diffed_against_the_old_one() {
        // Exercises the real decision function `Tracker::sample` itself calls, not a copy of its
        // logic: seed the cache as if an earlier real sample had happened, then ask `prev_for` the
        // question a second sample would ask it, with the pid's new (recycled) start time.
        let mut cache = HashMap::new();
        cache.insert(100, Prev { start_abstime: 111, at: Instant::now(), user_ns: 0, system_ns: 0, disk_read: Some(0), disk_write: Some(0) });
        assert!(prev_for(&cache, 100, 222).is_none(), "a different start_abstime for the same pid must not be treated as the same process");
        assert!(prev_for(&cache, 100, 111).is_some(), "the same start_abstime for the same pid is genuinely the same process");
        assert!(prev_for(&cache, 999, 111).is_none(), "a pid never seen before has no previous sample");
    }

    #[test]
    fn disk_unavailable_when_the_v0_fallback_has_no_disk_fields() {
        let u = build_usage(100, "x".into(), &raw(1000, 0, 0, 1_000_000, None), None, Instant::now(), (1, 1), Some(0));
        assert!(matches!(u.disk_read_bytes_per_sec, Some(Reading::Unavailable(_))));
        assert!(matches!(u.disk_write_bytes_per_sec, Some(Reading::Unavailable(_))));
    }

    #[test]
    fn another_users_process_is_restricted_with_ps_derived_cpu_and_memory_and_dash_disk() {
        let mut t = Tracker::default();
        let procs = vec![Proc { pid: 1, ppid: 0, user: "root".into(), cpu: 1.5, rss_kb: 4096, command: "launchd".into(), exe: "/sbin/launchd".into() }];
        let rows = t.sample(&procs, "not-root");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].restricted);
        assert_eq!(rows[0].cpu_pct, Reading::Value(1.5));
        assert_eq!(rows[0].mem_bytes, Reading::Value(4096 * 1024));
        assert!(matches!(rows[0].disk_read_bytes_per_sec, Some(Reading::Unavailable(_))));
        assert!(matches!(rows[0].start_unix, Reading::Unavailable(_)), "never ours to echo back on /api/kill");
    }

    #[test]
    fn start_unix_adds_boot_time_to_the_converted_abstime() {
        // 2_000_000_000 ns = 2 s at a 1:1 timebase.
        assert_eq!(start_unix(2_000_000_000, (1, 1), Some(1_700_000_000)), Reading::Value(1_700_000_002));
    }

    #[test]
    fn start_unix_is_unavailable_without_a_boot_time_reading() {
        assert!(matches!(start_unix(0, (1, 1), None), Reading::Unavailable(_)));
    }

    #[test]
    fn live_start_unix_reads_a_plausible_unix_timestamp_for_a_real_running_process() {
        // Not asserting "recent": this test binary's own reported start time depends on how the
        // process running it was launched (a sandboxed harness can reuse or wrap a process in ways
        // that make it older than the test itself), so the only thing genuinely true in every
        // environment is that a real process's start lies between boot and now.
        let pid = std::process::id();
        let Some(start) = live_start_unix(pid) else { panic!("could not read this process's own start time") };
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let boot = boottime_unix().expect("boottime_unix should read on a real Mac");
        assert!(start <= now, "a process cannot start in the future: start={start} now={now}");
        assert!(start >= boot, "a process cannot start before the machine booted: start={start} boot={boot}");
    }

    #[test]
    fn live_start_unix_is_none_for_a_pid_nothing_is_using() {
        // pid 0 is never a real user process on macOS, so pidrusage must fail for it.
        assert_eq!(live_start_unix(0), None);
    }

    /// Manual, real-machine timing check: not run by `cargo test` or CI (every process on the
    /// real machine, not a fixture). `cargo test -- --ignored real_full_pass_timing_on_this_mac`
    /// reproduces the cost-of-one-pass figure recorded in the Implementation Record.
    #[test]
    #[ignore]
    fn real_full_pass_timing_on_this_mac() {
        let mut t = Tracker::default();
        let me = crate::actions::my_username();
        let full_start = Instant::now();
        let procs = crate::collect::processes();
        let ps_elapsed = full_start.elapsed();
        let start = Instant::now();
        let rows = t.sample(&procs, &me);
        let elapsed = start.elapsed();
        println!("{} processes, {} rows, ps: {:?}, rusage pass: {:?}, total: {:?}", procs.len(), rows.len(), ps_elapsed, elapsed, full_start.elapsed());
    }

    #[test]
    fn sort_and_limit_orders_by_the_requested_column_and_caps_the_count() {
        let rows = vec![
            ProcessUsage { pid: 1, command: "a".into(), restricted: false, cpu_pct: Reading::Value(10.0), mem_bytes: Reading::Value(100), disk_read_bytes_per_sec: None, disk_write_bytes_per_sec: None, start_unix: Reading::Value(1000) },
            ProcessUsage { pid: 2, command: "b".into(), restricted: false, cpu_pct: Reading::Value(90.0), mem_bytes: Reading::Value(50), disk_read_bytes_per_sec: None, disk_write_bytes_per_sec: None, start_unix: Reading::Value(1000) },
        ];
        let by_cpu = sort_and_limit(rows.clone(), "cpu", 1);
        assert_eq!(by_cpu.len(), 1);
        assert_eq!(by_cpu[0].pid, 2);
        let by_mem = sort_and_limit(rows, "mem", 10);
        assert_eq!(by_mem[0].pid, 1);
    }
}
