//! Where the disk space went: a bounded scan of a fixed list of locations on the boot volume, a
//! label for each item from a rules table, and the capacity split into used, purgeable and free.
//! Read-only: nothing here removes or moves a file. The scan reads metadata only (`lstat`), never
//! follows a symlink, never crosses a device and never walks the whole disk.
use crate::sampler::Reading;
use serde::Serialize;
use std::collections::HashSet;
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Total time for one scan. A slow scan would make the monitor its own top consumer.
const TIME_BUDGET: Duration = Duration::from_secs(20);
/// Kept back for `tmutil` and the reply, so the whole command finishes within `TIME_BUDGET`.
const RESERVE: Duration = Duration::from_secs(1);
const MAX_ENTRIES_PER_LOCATION: u64 = 200_000;
const NODE_MODULES_DEPTH: usize = 6;
const MAX_NODE_MODULES: usize = 300;
const IDLE_SECS: u64 = 90 * 24 * 3600;
/// Folders that hold the user's own files: nothing under them can be `safe`.
const USER_CONTENT: &[&str] = &["Documents", "Desktop", "Pictures", "Movies", "Music", "Downloads"];
const TMUTIL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Label {
    /// An app rebuilds or re-downloads it.
    Safe,
    /// Removing it has side effects the user should weigh.
    Review,
    /// The user's own files.
    Yours,
}

/// The label for one path and the name of the rule that gave it.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct Verdict { pub label: Label, pub rule: &'static str }

fn verdict(label: Label, rule: &'static str) -> Verdict { Verdict { label, rule } }

/// The rules table. Every path gets exactly one label; the first matching rule wins and the order
/// puts the user's files before anything else, so no allowlist entry can reach into them. Only rules
/// decide this: a model never sets a label. `project_idle` says whether the project that owns a
/// `node_modules` folder has been untouched for 90 days.
pub fn label_for(home: &Path, path: &Path, project_idle: bool) -> Verdict {
    // `a/../b` would match a prefix rule for `a` and then point somewhere else.
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return verdict(Label::Review, "unclear-path");
    }
    let Ok(rel) = path.strip_prefix(home) else {
        return if path.starts_with("/Applications") { verdict(Label::Review, "applications") } else { verdict(Label::Review, "outside-home") };
    };
    let parts: Vec<&str> = rel.iter().filter_map(|p| p.to_str()).collect();
    let under = |prefix: &[&str]| parts.len() >= prefix.len() && parts[..prefix.len()] == *prefix;
    match parts.as_slice() {
        [first, ..] if USER_CONTENT.contains(first) => verdict(Label::Yours, "user-content"),
        _ if under(&["Library", "Mail"]) => verdict(Label::Yours, "mail"),
        _ if under(&["Library", "Developer", "Xcode", "DerivedData"]) => verdict(Label::Safe, "xcode-derived-data"),
        _ if under(&["Library", "Caches", "Homebrew"]) => verdict(Label::Safe, "homebrew-cache"),
        _ if parts.len() >= 3 && under(&["Library", "Caches"]) => verdict(Label::Safe, "app-cache"),
        _ if parts.contains(&"node_modules") => if project_idle { verdict(Label::Safe, "idle-node-modules") } else { verdict(Label::Review, "node-modules") },
        _ if under(&["Library", "Containers", "com.docker.docker"]) => verdict(Label::Review, "docker-data"),
        _ if under(&["Library", "Developer", "CoreSimulator"]) => verdict(Label::Review, "simulators"),
        _ if under(&["Library", "Logs"]) => verdict(Label::Review, "logs"),
        _ => verdict(Label::Review, "unclassified"),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits { pub deadline: Instant, pub max_entries: u64 }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stop { EntryCap, Time }

#[derive(Default, Debug)]
pub struct Walked {
    /// `st_blocks * 512`: what the files take on disk, so a sparse file is not over-counted.
    pub allocated: u64,
    pub apparent: u64,
    pub entries: u64,
    /// Subfolders or entries that could not be read; their size is missing from the total.
    pub unreadable: u32,
    pub stop: Option<Stop>,
}

fn io_reason(e: &io::Error) -> &'static str {
    // Without Full Disk Access macOS answers EPERM (not EACCES) for protected folders.
    match e.raw_os_error() {
        Some(libc::EACCES) | Some(libc::EPERM) => "permission denied",
        _ => "could not be read",
    }
}

/// Sizes one location. `None` when it does not exist (the caller omits it, never shows 0).
pub fn walk(root: &Path, lim: Limits) -> Option<Result<Walked, &'static str>> {
    let meta = match std::fs::symlink_metadata(root) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => return Some(Err(io_reason(&e))),
    };
    let mut w = Walked::default();
    count(&mut w, &meta);
    if !meta.is_dir() { return Some(Ok(w)); }
    let dev = meta.dev();
    // An unreadable top folder must read as unavailable, not as an empty folder.
    if let Err(e) = std::fs::read_dir(root) { return Some(Err(io_reason(&e))); }
    let mut pending = vec![root.to_path_buf()];
    let mut linked: HashSet<(u64, u64)> = HashSet::new();
    let mut since_clock = 0u32;
    while let Some(dir) = pending.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { w.unreadable += 1; continue };
        for entry in rd {
            let Ok(entry) = entry else { w.unreadable += 1; continue };
            w.entries += 1;
            if w.entries > lim.max_entries { w.stop = Some(Stop::EntryCap); return Some(Ok(w)); }
            since_clock += 1;
            if since_clock >= 16 {
                since_clock = 0;
                if Instant::now() >= lim.deadline { w.stop = Some(Stop::Time); return Some(Ok(w)); }
                pace();
            }
            // `DirEntry::metadata` is an `lstat`: a symlink is sized as itself and never followed.
            let Ok(m) = entry.metadata() else { w.unreadable += 1; continue };
            if m.is_dir() {
                if m.dev() != dev { continue; }
                count(&mut w, &m);
                pending.push(entry.path());
            } else if m.nlink() > 1 && !linked.insert((m.dev(), m.ino())) {
                // A second name for a file already counted.
            } else {
                count(&mut w, &m);
            }
        }
    }
    if Instant::now() >= lim.deadline { w.stop = Some(Stop::Time); }
    Some(Ok(w))
}

/// Share of wall time the scan may spend working. Without a limit a scan of a developer home used
/// about 45% of one core, which would make the monitor its own top consumer.
const DUTY: f64 = 0.2;
/// Rests shorter than this are skipped: a short sleep on a low-priority thread overshoots a lot.
const MIN_REST: Duration = Duration::from_millis(20);
const MAX_REST: Duration = Duration::from_millis(250);

struct Pacer { start: Instant, cpu_start: Duration }

thread_local!(static PACER: std::cell::RefCell<Option<Pacer>> = const { std::cell::RefCell::new(None) });

fn pace_reset() { PACER.with(|p| *p.borrow_mut() = None); }

/// CPU time this thread has used. Wall time would count disk waits as work and rest too much.
fn thread_cpu() -> Duration {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid out-pointer for the call; on failure it stays zero and pacing does nothing.
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    Duration::new(ts.tv_sec.max(0) as u64, ts.tv_nsec.clamp(0, 999_999_999) as u32)
}

/// Called every few entries: rests when the scan has used more than `DUTY` of one core since it began.
/// The rest is computed from what has really elapsed, so a sleep that overshoots shortens the next one.
fn pace() {
    PACER.with(|cell| {
        let mut slot = cell.borrow_mut();
        let p = slot.get_or_insert_with(|| Pacer { start: Instant::now(), cpu_start: thread_cpu() });
        let used = thread_cpu().saturating_sub(p.cpu_start);
        let wanted = used.div_f64(DUTY).saturating_sub(p.start.elapsed());
        if wanted >= MIN_REST { std::thread::sleep(wanted.min(MAX_REST)); }
    });
}

fn count(w: &mut Walked, m: &std::fs::Metadata) {
    w.allocated += m.blocks() * 512;
    if m.is_file() { w.apparent += m.len(); }
}

#[derive(Serialize, Debug)]
pub struct Item {
    pub path: String,
    pub category: &'static str,
    pub label: Label,
    pub rule: &'static str,
    /// Allocated bytes, or why they could not be read.
    pub size: Reading<u64>,
    /// Bytes the files claim to hold; larger than `size` for a sparse file. Absent when `size` is.
    pub apparent_bytes: Option<u64>,
    pub entries: u64,
    /// False when the entry cap or the time budget stopped this item early, so `size` is a lower bound.
    pub complete: bool,
    pub unreadable: u32,
}

#[derive(Serialize, Debug)]
pub struct Category { pub category: &'static str, pub bytes: u64, pub items: usize, pub unavailable_items: usize }

#[derive(Serialize, Debug)]
pub struct Capacity { pub total: Reading<u64>, pub used: Reading<u64>, pub purgeable: Reading<u64>, pub free: Reading<u64> }

#[derive(Serialize, Debug)]
pub struct Snapshots { pub count: Reading<usize>, pub dates: Vec<String>, pub size: Reading<u64> }

#[derive(Serialize, Debug)]
pub struct Storage {
    pub scanned_at: u64,
    pub scan_ms: u64,
    pub complete: bool,
    /// Locations the time budget ended the scan before reaching.
    pub not_reached: Vec<String>,
    pub capacity: Capacity,
    pub categories: Vec<Category>,
    pub items: Vec<Item>,
    pub snapshots: Snapshots,
}

struct Location { path: PathBuf, category: &'static str, children: bool }

fn locations(home: &Path) -> Vec<Location> {
    let at = |rel: &str, category, children| Location { path: home.join(rel), category, children };
    vec![
        at("Library/Developer/Xcode/DerivedData", "developer", false),
        at("Library/Developer/CoreSimulator", "developer", false),
        at("Library/Caches", "caches", true),
        at("Library/Containers/com.docker.docker", "developer", false),
        at("Library/Logs", "logs", false),
        at("Downloads", "downloads", false),
        at("Library/Mail", "mail", false),
    ]
}

struct Scan<'a> { home: &'a Path, lim: Limits, items: Vec<Item>, not_reached: Vec<String>, partial: bool }

impl Scan<'_> {
    fn measure(&mut self, path: &Path, category: &'static str, project_idle: bool) {
        if Instant::now() >= self.lim.deadline {
            self.not_reached.push(path.display().to_string());
            return;
        }
        let Some(result) = walk(path, self.lim) else { return };
        let v = label_for(self.home, path, project_idle);
        let item = |size, apparent_bytes, entries, complete, unreadable| Item {
            path: path.display().to_string(), category, label: v.label, rule: v.rule, size, apparent_bytes, entries, complete, unreadable,
        };
        self.items.push(match result {
            Ok(w) => {
                self.partial |= w.stop.is_some();
                item(Reading::Value(w.allocated), Some(w.apparent), w.entries, w.stop.is_none(), w.unreadable)
            }
            Err(reason) => item(Reading::Unavailable(reason), None, 0, false, 0),
        });
    }
}

/// Folders under `home` named `node_modules`, to `NODE_MODULES_DEPTH` levels, with whether the
/// owning project looks idle. Skips `~/Library`, dot-folders and the other scanned roots so nothing is counted twice.
fn find_node_modules(home: &Path, lim: Limits, now: u64) -> (Vec<(PathBuf, bool)>, bool) {
    let mut found = Vec::new();
    let mut pending = vec![(home.to_path_buf(), 0usize)];
    let (mut visited, mut cut) = (0u64, false);
    while let Some((dir, depth)) = pending.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            visited += 1;
            if visited > lim.max_entries || (visited % 16 == 0 && Instant::now() >= lim.deadline) { return (found, true); }
            if visited % 16 == 0 { pace(); }
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_dir() { continue; }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == "node_modules" {
                if found.len() >= MAX_NODE_MODULES { cut = true; continue; }
                let idle = project_idle(&dir, &entry.path(), now);
                found.push((entry.path(), idle));
            } else if depth + 1 < NODE_MODULES_DEPTH && !name.starts_with('.') && !(depth == 0 && matches!(&*name, "Library" | "Downloads")) {
                pending.push((entry.path(), depth + 1));
            }
        }
    }
    (found, cut)
}

/// A cheap proxy for "nobody works here": no change in 90 days to the project folder, its
/// `node_modules`, `package.json` or the git index. Editing a source file alone does not move the
/// folder's time, which is why the git files are read too.
fn project_idle(project: &Path, modules: &Path, now: u64) -> bool {
    let mtime = |p: &Path| std::fs::symlink_metadata(p).ok().map(|m| m.mtime().max(0) as u64);
    let newest = [project.to_path_buf(), modules.to_path_buf(), project.join("package.json"), project.join(".git/index"), project.join(".git/HEAD")]
        .iter().filter_map(|p| mtime(p)).max().unwrap_or(now);
    now.saturating_sub(newest) >= IDLE_SECS
}

/// Everything except capacity and snapshots, which describe the volume rather than a folder.
fn scan_items(home: &Path, applications: &Path, lim: Limits, now: u64) -> (Vec<Item>, Vec<String>, bool) {
    let mut s = Scan { home, lim, items: Vec::new(), not_reached: Vec::new(), partial: false };
    for loc in locations(home) {
        let targets = match (loc.children, std::fs::read_dir(&loc.path)) {
            (true, Ok(rd)) => {
                let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
                v.sort();
                v
            }
            _ => vec![loc.path.clone()],
        };
        for t in targets { s.measure(&t, loc.category, false); }
    }
    let (modules, cut) = if Instant::now() >= lim.deadline { s.not_reached.push("node_modules search".into()); (Vec::new(), false) } else { find_node_modules(home, lim, now) };
    s.partial |= cut;
    for (path, idle) in modules { s.measure(&path, "developer", idle); }
    // Last: it cannot be `safe`, so it is the least useful place to spend the time budget.
    s.measure(applications, "applications", false);
    s.partial |= !s.not_reached.is_empty();
    (s.items, s.not_reached, s.partial)
}

fn categories(items: &[Item]) -> Vec<Category> {
    let mut out: Vec<Category> = Vec::new();
    for i in items {
        let at = match out.iter().position(|c| c.category == i.category) {
            Some(at) => at,
            None => { out.push(Category { category: i.category, bytes: 0, items: 0, unavailable_items: 0 }); out.len() - 1 }
        };
        out[at].items += 1;
        match i.size { Reading::Value(b) => out[at].bytes += b, _ => out[at].unavailable_items += 1 }
    }
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    out
}

/// `statfs` gives total and free (what `df` shows); the "important usage" figure adds space macOS
/// would reclaim on demand. Their difference is purgeable. `volume` is (available, important).
fn split_capacity(statfs: Option<(u64, u64)>, volume: Option<(u64, u64)>) -> Capacity {
    let Some((total, used_with_purgeable)) = statfs else {
        let gone = || Reading::Unavailable("statfs failed");
        return Capacity { total: gone(), used: gone(), purgeable: gone(), free: gone() };
    };
    let free = total.saturating_sub(used_with_purgeable);
    match volume {
        Some((available, important)) => {
            let purgeable = important.saturating_sub(available).min(used_with_purgeable);
            Capacity { total: Reading::Value(total), used: Reading::Value(used_with_purgeable - purgeable), purgeable: Reading::Value(purgeable), free: Reading::Value(free) }
        }
        // Without the purgeable split, `used` still counts it.
        None => Capacity { total: Reading::Value(total), used: Reading::Value(used_with_purgeable), purgeable: Reading::Unavailable("volume capacity keys unreadable"), free: Reading::Value(free) },
    }
}

mod volume {
    use core_foundation_sys::base::{kCFAllocatorDefault, Boolean, CFGetTypeID, CFRelease, CFTypeRef};
    use core_foundation_sys::error::CFErrorRef;
    use core_foundation_sys::number::{kCFNumberSInt64Type, CFNumberGetTypeID, CFNumberGetValue, CFNumberRef};
    use core_foundation_sys::string::{kCFStringEncodingUTF8, CFStringCreateWithCString, CFStringRef};
    use core_foundation_sys::url::{kCFURLPOSIXPathStyle, CFURLCreateWithFileSystemPath, CFURLRef};

    extern "C" {
        // Declared here because core-foundation-sys leaves it out; CoreFoundation is already linked.
        fn CFURLCopyResourcePropertyForKey(url: CFURLRef, key: CFStringRef, value: *mut CFTypeRef, error: *mut CFErrorRef) -> Boolean;
    }

    /// One integer volume property of `/` by its `NSURLVolume...` key, readable without root.
    // SAFETY: every Core Foundation object created here is released on each path; the key string and
    // URL outlive the call; `value` is read as a number only after its type id is checked.
    fn read(key: &str) -> Option<u64> {
        let key = std::ffi::CString::new(key).ok()?;
        unsafe {
            let k = CFStringCreateWithCString(kCFAllocatorDefault, key.as_ptr(), kCFStringEncodingUTF8);
            let root = CFStringCreateWithCString(kCFAllocatorDefault, c"/".as_ptr(), kCFStringEncodingUTF8);
            let url = CFURLCreateWithFileSystemPath(kCFAllocatorDefault, root, kCFURLPOSIXPathStyle, 1);
            if !root.is_null() {
                CFRelease(root as CFTypeRef);
            }
            let mut out = None;
            if !k.is_null() && !url.is_null() {
                let mut v: CFTypeRef = std::ptr::null();
                let mut err: CFErrorRef = std::ptr::null_mut();
                if CFURLCopyResourcePropertyForKey(url, k, &mut v, &mut err) != 0 && !v.is_null() {
                    if CFGetTypeID(v) == CFNumberGetTypeID() {
                        let mut n: i64 = 0;
                        if CFNumberGetValue(v as CFNumberRef, kCFNumberSInt64Type, &mut n as *mut i64 as *mut _) {
                            out = u64::try_from(n).ok();
                        }
                    }
                    CFRelease(v);
                }
                if !err.is_null() { CFRelease(err as CFTypeRef); }
            }
            if !k.is_null() { CFRelease(k as CFTypeRef); }
            out
        }
    }

    /// (available, important-usage) bytes for `/`.
    pub fn available_and_important() -> Option<(u64, u64)> {
        Some((read("NSURLVolumeAvailableCapacityKey")?, read("NSURLVolumeAvailableCapacityForImportantUsageKey")?))
    }
}

/// `tmutil` output -> snapshot dates (`YYYY-MM-DD-HHMMSS`). OS-update snapshots are not Time Machine's and are skipped.
fn parse_snapshots(out: &str) -> Vec<String> {
    out.lines().filter_map(|l| l.trim().strip_prefix("com.apple.TimeMachine.")?.strip_suffix(".local").map(str::to_string)).collect()
}

fn run_tmutil() -> Result<String, &'static str> {
    let mut child = Command::new("/usr/bin/tmutil").args(["listlocalsnapshots", "/"]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        .map_err(|e| if e.kind() == io::ErrorKind::NotFound { "tmutil not found" } else { "tmutil could not start" })?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if started.elapsed() < TMUTIL_TIMEOUT => std::thread::sleep(Duration::from_millis(25)),
            _ => { let _ = child.kill(); let _ = child.wait(); return Err("tmutil timed out"); }
        }
    };
    if !status.success() { return Err("tmutil failed"); }
    let mut text = String::new();
    child.stdout.take().ok_or("tmutil gave no output")?.read_to_string(&mut text).map_err(|_| "tmutil output unreadable")?;
    Ok(text)
}

fn snapshots(out: Result<String, &'static str>) -> Snapshots {
    match out {
        Ok(text) => {
            let dates = parse_snapshots(&text);
            // As of 2026-11, neither `tmutil` nor `diskutil apfs listSnapshots` reports a size.
            Snapshots { count: Reading::Value(dates.len()), dates, size: Reading::Unavailable("no tool reports a snapshot size without root") }
        }
        Err(reason) => Snapshots { count: Reading::Unavailable(reason), dates: Vec::new(), size: Reading::Unavailable(reason) },
    }
}

extern "C" {
    fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
}

/// `QOS_CLASS_UTILITY` from `<sys/qos.h>`.
const QOS_UTILITY: u32 = 0x11;

/// Lowers this thread's CPU and disk priority so the scan yields to the user's work. The utility
/// class, not the background one: the background class throttles disk reads so hard that a scan
/// reached about 5,000 entries in 27 s on a developer home.
fn run_in_background() {
    // SAFETY: sets the QoS class of the calling thread only; a failure leaves the normal priority.
    unsafe {
        pthread_set_qos_class_self_np(QOS_UTILITY, 0);
    }
}

fn home() -> PathBuf { PathBuf::from(std::env::var("HOME").unwrap_or_default()) }

fn now_secs() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

/// One full scan. Blocking: call it off the request thread.
pub fn scan() -> Storage {
    run_in_background();
    pace_reset();
    let started = Instant::now();
    let lim = Limits { deadline: started + TIME_BUDGET - RESERVE, max_entries: MAX_ENTRIES_PER_LOCATION };
    let (items, not_reached, partial) = scan_items(&home(), Path::new("/Applications"), lim, now_secs());
    let capacity = split_capacity(crate::iokit::boot_volume_capacity(), volume::available_and_important());
    Storage {
        scanned_at: now_secs(), scan_ms: started.elapsed().as_millis() as u64, complete: !partial, not_reached,
        capacity, categories: categories(&items), items, snapshots: snapshots(run_tmutil()),
    }
}

#[derive(Serialize)]
struct View<'a> {
    scanning: bool,
    #[serde(flatten)]
    result: Option<&'a Storage>,
}

pub fn to_json(result: Option<&Storage>, scanning: bool, pretty: bool) -> String {
    let v = View { scanning, result };
    (if pretty { serde_json::to_string_pretty(&v) } else { serde_json::to_string(&v) }).unwrap_or_default()
}

#[derive(Default)]
struct State { result: Option<Storage>, scanning: bool }

/// The latest scan, kept so a request never waits for one.
#[derive(Default)]
pub struct Cache(Mutex<State>);

/// Clears `scanning` even if the scan panics, so one failure cannot stop every later scan.
struct Done(Arc<Cache>);
impl Drop for Done {
    fn drop(&mut self) { self.0 .0.lock().unwrap_or_else(|e| e.into_inner()).scanning = false; }
}

impl Cache {
    /// The cached result as JSON. The first request, and `fresh`, start a scan on another thread unless
    /// one is running; the reply is always the previous result with `scanning` saying whether a scan is under way.
    pub fn request(self: &Arc<Self>, fresh: bool, scan: fn() -> Storage) -> String {
        let mut st = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let start = (fresh || st.result.is_none()) && !st.scanning;
        if start { st.scanning = true; }
        let json = to_json(st.result.as_ref(), st.scanning, false);
        drop(st);
        if start {
            let cache = self.clone();
            std::thread::spawn(move || {
                let guard = Done(cache.clone());
                let result = scan();
                cache.0.lock().unwrap_or_else(|e| e.into_inner()).result = Some(result);
                drop(guard);
            });
        }
        json
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::sync::atomic::{AtomicUsize, Ordering};

    const HOME: &str = "/h";

    fn label(rel: &str, idle: bool) -> Verdict { label_for(Path::new(HOME), &Path::new(HOME).join(rel), idle) }

    fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ml-storage-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn lim(max_entries: u64) -> Limits { Limits { deadline: Instant::now() + Duration::from_secs(30), max_entries } }

    #[test]
    fn allowlisted_locations_are_safe() {
        for (rel, rule) in [
            ("Library/Developer/Xcode/DerivedData", "xcode-derived-data"),
            ("Library/Developer/Xcode/DerivedData/App-abc/Build", "xcode-derived-data"),
            ("Library/Caches/Homebrew", "homebrew-cache"),
            ("Library/Caches/com.example.app", "app-cache"),
            ("Library/Caches/com.example.app/deep/file", "app-cache"),
        ] {
            assert_eq!(label(rel, false), verdict(Label::Safe, rule), "{rel}");
        }
    }

    #[test]
    fn user_content_is_never_safe_even_when_a_safe_pattern_matches_inside_it() {
        for dir in USER_CONTENT {
            for tail in ["", "/x", "/Library/Caches/app", "/proj/node_modules", "/proj/node_modules/pkg", "/Library/Developer/Xcode/DerivedData", "/Library/Caches/Homebrew"] {
                for idle in [false, true] {
                    let v = label(&format!("{dir}{tail}"), idle);
                    assert_eq!(v.label, Label::Yours, "{dir}{tail}");
                }
            }
        }
    }

    #[test]
    fn a_folder_that_only_starts_with_a_user_folder_name_is_not_user_content() {
        assert_eq!(label("Documents2/node_modules", true).label, Label::Safe);
        assert_eq!(label("Downloads-old", false).label, Label::Review);
    }

    #[test]
    fn mail_is_yours_and_unknown_paths_need_review() {
        assert_eq!(label("Library/Mail", false), verdict(Label::Yours, "mail"));
        assert_eq!(label("Library/Mail/V10/x", true).label, Label::Yours);
        for rel in ["Library/Logs", "Library/Logs/DiagnosticReports", "Library/Containers/com.docker.docker/Data", "Library/Developer/CoreSimulator/Devices", "Library/Caches", "Something/Else"] {
            assert_eq!(label(rel, false).label, Label::Review, "{rel}");
        }
        assert_eq!(label_for(Path::new(HOME), Path::new("/Applications"), false).label, Label::Review);
        assert_eq!(label_for(Path::new(HOME), Path::new("/var/db/x"), false).rule, "outside-home");
    }

    #[test]
    fn node_modules_is_safe_only_when_the_project_is_idle() {
        assert_eq!(label("code/app/node_modules", true), verdict(Label::Safe, "idle-node-modules"));
        assert_eq!(label("code/app/node_modules", false), verdict(Label::Review, "node-modules"));
        assert_eq!(label("code/app/node_modules/.bin/x", true).label, Label::Safe);
    }

    #[test]
    fn a_path_that_climbs_out_of_a_safe_prefix_is_never_safe() {
        for p in ["/h/Library/Caches/../Documents/x", "/h/Library/Caches/app/../../Documents", "/h/Library/Developer/Xcode/DerivedData/../../../../Desktop", "relative/Library/Caches/app"] {
            assert_eq!(label_for(Path::new(HOME), Path::new(p), true), verdict(Label::Review, "unclear-path"), "{p}");
        }
    }

    #[test]
    fn allocated_size_is_blocks_not_apparent_size_for_a_sparse_file() {
        let d = temp("sparse");
        let f = fs::File::create(d.join("Docker.raw")).unwrap();
        f.set_len(200 * 1024 * 1024).unwrap();
        let w = walk(&d, lim(1000)).unwrap().unwrap();
        assert_eq!(w.apparent, 200 * 1024 * 1024);
        assert!(w.allocated < 10 * 1024 * 1024, "allocated {} should be far below the apparent size", w.allocated);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_symlink_loop_terminates_and_a_link_to_a_big_folder_is_not_followed() {
        let d = temp("loop");
        let outside = temp("outside");
        fs::write(outside.join("big"), vec![1u8; 1024 * 1024]).unwrap();
        fs::create_dir(d.join("a")).unwrap();
        symlink(&d, d.join("a/up")).unwrap();
        symlink(d.join("a"), d.join("a/self")).unwrap();
        symlink(&outside, d.join("to-outside")).unwrap();
        let w = walk(&d, lim(1000)).unwrap().unwrap();
        assert_eq!(w.entries, 4, "a, up, self and to-outside, each once");
        assert!(w.allocated < 512 * 1024, "the 1 MB file behind the link must not be counted: {}", w.allocated);
        assert_eq!(w.stop, None);
        fs::remove_dir_all(&d).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn a_hard_linked_file_is_counted_once() {
        let d = temp("hardlink");
        fs::write(d.join("one"), vec![1u8; 256 * 1024]).unwrap();
        fs::hard_link(d.join("one"), d.join("two")).unwrap();
        let w = walk(&d, lim(1000)).unwrap().unwrap();
        assert!(w.allocated < 400 * 1024, "{}", w.allocated);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_time_budget_stops_the_walk_and_says_so() {
        let d = temp("budget");
        for i in 0..200 { fs::write(d.join(format!("f{i}")), b"x").unwrap(); }
        let past = Limits { deadline: Instant::now() - Duration::from_secs(1), max_entries: u64::MAX };
        let w = walk(&d, past).unwrap().unwrap();
        assert_eq!(w.stop, Some(Stop::Time));
        assert!(w.entries < 200, "stopped early: {}", w.entries);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_entry_cap_stops_the_walk_and_marks_the_item_incomplete() {
        let d = temp("cap");
        for i in 0..50 { fs::write(d.join(format!("f{i}")), b"x").unwrap(); }
        let w = walk(&d, lim(10)).unwrap().unwrap();
        assert_eq!(w.stop, Some(Stop::EntryCap));
        let home = temp("cap-home");
        fs::create_dir_all(home.join("Downloads")).unwrap();
        for i in 0..50 { fs::write(home.join("Downloads").join(format!("f{i}")), b"x").unwrap(); }
        let (items, not_reached, partial) = scan_items(&home, &home.join("none"), lim(10), now_secs());
        assert!(partial && not_reached.is_empty());
        assert!(!items.iter().find(|i| i.category == "downloads").unwrap().complete);
        fs::remove_dir_all(&d).unwrap();
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn an_expired_budget_lists_the_locations_not_reached() {
        let home = temp("expired");
        fs::create_dir_all(home.join("Downloads")).unwrap();
        let past = Limits { deadline: Instant::now() - Duration::from_secs(1), max_entries: 100 };
        let (items, not_reached, partial) = scan_items(&home, &home.join("none"), past, now_secs());
        assert!(items.is_empty() && partial);
        assert!(not_reached.iter().any(|p| p.ends_with("Downloads")) && not_reached.iter().any(|p| p == "node_modules search"));
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_denied_folder_is_unavailable_not_zero() {
        let d = temp("denied");
        let locked = d.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("f"), b"data").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let r = walk(&locked, lim(100)).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(r.unwrap_err(), "permission denied");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_denied_subfolder_is_counted_and_the_rest_is_still_sized() {
        let d = temp("denied-sub");
        fs::write(d.join("ok"), vec![1u8; 64 * 1024]).unwrap();
        let locked = d.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let w = walk(&d, lim(100)).unwrap().unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(w.unreadable, 1);
        assert!(w.allocated >= 64 * 1024);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_missing_location_is_omitted_and_a_denied_one_is_listed() {
        let home = temp("omit");
        let locked = home.join("Library/Mail");
        fs::create_dir_all(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let (items, _, _) = scan_items(&home, &home.join("none"), lim(100), now_secs());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!items.iter().any(|i| i.path.contains("Docker") || i.path.contains("DerivedData")), "missing locations are omitted");
        let mail = items.iter().find(|i| i.path.ends_with("Library/Mail")).unwrap();
        assert_eq!(mail.size, Reading::Unavailable("permission denied"));
        assert_eq!(mail.label, Label::Yours);
        assert_eq!(mail.apparent_bytes, None);
        fs::remove_dir_all(&home).unwrap();
    }

    /// Mounts a tiny disk image inside a folder and checks the walk stays on the folder's own volume.
    /// Skipped, with a note, where `hdiutil` cannot mount without a prompt.
    #[test]
    fn a_walk_never_crosses_into_another_volume() {
        struct Detach(PathBuf);
        impl Drop for Detach {
            fn drop(&mut self) {
                let _ = Command::new("/usr/bin/hdiutil").args(["detach", "-force"]).arg(&self.0).output();
            }
        }
        let d = temp("device");
        let ok = |c: &mut Command| c.output().map(|o| o.status.success()).unwrap_or(false);
        let image = d.join("v.dmg");
        let mount = d.join("root/mnt");
        fs::create_dir_all(&mount).unwrap();
        if !ok(Command::new("/usr/bin/hdiutil").args(["create", "-size", "4m", "-fs", "HFS+", "-volname", "t", "-ov"]).arg(&image))
            || !ok(Command::new("/usr/bin/hdiutil").args(["attach", "-nobrowse", "-mountpoint"]).arg(&mount).arg(&image))
        {
            eprintln!("skipped: could not mount a test disk image");
            fs::remove_dir_all(&d).unwrap();
            return;
        }
        let _detach = Detach(mount.clone());
        fs::write(mount.join("big"), vec![1u8; 1024 * 1024]).unwrap();
        fs::write(d.join("root/own"), b"x").unwrap();
        let w = walk(&d.join("root"), lim(1000)).unwrap().unwrap();
        assert_eq!(w.entries, 2, "`own` and the mount point, whose contents are not visited");
        assert!(w.allocated < 512 * 1024, "{}", w.allocated);
        drop(_detach);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn caches_are_listed_per_app_and_node_modules_are_found_with_idleness() {
        let home = temp("layout");
        fs::create_dir_all(home.join("Library/Caches/com.a/sub")).unwrap();
        fs::write(home.join("Library/Caches/com.a/sub/f"), vec![1u8; 4096]).unwrap();
        fs::create_dir_all(home.join("Library/Caches/com.b")).unwrap();
        fs::create_dir_all(home.join("code/app/node_modules/pkg")).unwrap();
        fs::create_dir_all(home.join("code/app/node_modules/pkg/node_modules")).unwrap();
        fs::create_dir_all(home.join(".hidden/node_modules")).unwrap();
        fs::create_dir_all(home.join("Library/x/node_modules")).unwrap();
        let (items, _, partial) = scan_items(&home, &home.join("none"), lim(1000), now_secs());
        assert!(!partial);
        let paths: Vec<_> = items.iter().map(|i| i.path.replace(&home.display().to_string(), "~")).collect();
        assert_eq!(paths.iter().filter(|p| p.starts_with("~/Library/Caches/")).count(), 2, "{paths:?}");
        assert_eq!(paths.iter().filter(|p| p.ends_with("node_modules")).count(), 1, "one project folder, no nested, hidden or Library: {paths:?}");
        let nm = items.iter().find(|i| i.path.ends_with("app/node_modules")).unwrap();
        assert_eq!(nm.label, Label::Review, "fresh project is not idle");
        assert!(items.iter().filter(|i| i.category == "caches").all(|i| i.label == Label::Safe));
        // 91 days later the same project reads as idle.
        let later = now_secs() + 91 * 24 * 3600;
        let (items, _, _) = scan_items(&home, &home.join("none"), lim(1000), later);
        assert_eq!(items.iter().find(|i| i.path.ends_with("app/node_modules")).unwrap().label, Label::Safe);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn category_totals_sum_values_and_count_unavailable_items() {
        let item = |category, size| Item { path: "p".into(), category, label: Label::Review, rule: "r", size, apparent_bytes: None, entries: 0, complete: true, unreadable: 0 };
        let c = categories(&[item("a", Reading::Value(5)), item("a", Reading::Value(7)), item("b", Reading::Unavailable("permission denied"))]);
        assert_eq!((c[0].category, c[0].bytes, c[0].items, c[0].unavailable_items), ("a", 12, 2, 0));
        assert_eq!((c[1].category, c[1].bytes, c[1].unavailable_items), ("b", 0, 1));
    }

    #[test]
    fn capacity_splits_purgeable_from_free_and_says_why_when_it_cannot() {
        let c = split_capacity(Some((1000, 600)), Some((300, 450)));
        assert_eq!((c.total, c.free, c.purgeable, c.used), (Reading::Value(1000), Reading::Value(400), Reading::Value(150), Reading::Value(450)));
        let c = split_capacity(Some((1000, 600)), None);
        assert_eq!((c.free, c.used), (Reading::Value(400), Reading::Value(600)));
        assert!(matches!(c.purgeable, Reading::Unavailable(r) if r.contains("unreadable")));
        // An estimate that exceeds what is used is clamped, never negative.
        let c = split_capacity(Some((1000, 100)), Some((900, 5000)));
        assert_eq!(c.purgeable, Reading::Value(100));
        assert!(matches!(split_capacity(None, None).total, Reading::Unavailable("statfs failed")));
    }

    #[test]
    fn the_real_volume_reports_important_usage_at_least_the_available_space() {
        let (available, important) = volume::available_and_important().expect("readable without root");
        assert!(available > 0 && important >= available / 2, "{available} {important}");
    }

    #[test]
    fn snapshots_parse_dates_and_say_why_when_tmutil_fails() {
        let out = "Snapshots for disk /:\ncom.apple.TimeMachine.2026-11-25-101500.local\ncom.apple.os.update-ABC\ncom.apple.TimeMachine.2026-11-26-091500.local\n";
        let s = snapshots(Ok(out.into()));
        assert_eq!((s.count, s.dates), (Reading::Value(2), vec!["2026-11-25-101500".to_string(), "2026-11-26-091500".to_string()]));
        assert!(matches!(s.size, Reading::Unavailable(_)));
        let s = snapshots(Err("tmutil not found"));
        assert_eq!((s.count, s.size), (Reading::Unavailable("tmutil not found"), Reading::Unavailable("tmutil not found")));
        assert_eq!(snapshots(Ok(String::new())).count, Reading::Value(0));
    }

    static SCANS: AtomicUsize = AtomicUsize::new(0);
    fn slow_fake_scan() -> Storage {
        SCANS.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(300));
        fake_storage()
    }

    fn wait_until_idle(cache: &Cache) {
        let until = Instant::now() + Duration::from_secs(10);
        while cache.0.lock().unwrap().scanning && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn fake_storage() -> Storage {
        Storage {
            scanned_at: 1, scan_ms: 300, complete: true, not_reached: vec![], capacity: split_capacity(None, None),
            categories: vec![], items: vec![], snapshots: snapshots(Err("tmutil not found")),
        }
    }

    #[test]
    fn two_requests_at_once_start_one_scan_and_the_reply_never_waits() {
        let cache = Arc::new(Cache::default());
        let t = Instant::now();
        let a = cache.request(false, slow_fake_scan);
        let b = cache.request(true, slow_fake_scan);
        assert!(t.elapsed() < Duration::from_millis(200), "the request thread must not wait for the scan");
        for json in [&a, &b] {
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            assert_eq!(v["scanning"], true);
            assert!(v.get("items").is_none(), "no previous result yet");
        }
        wait_until_idle(&cache);
        assert_eq!(SCANS.load(Ordering::SeqCst), 1);
        let v: serde_json::Value = serde_json::from_str(&cache.request(false, slow_fake_scan)).unwrap();
        assert_eq!((v["scanning"].clone(), v["scanned_at"].clone()), (false.into(), 1.into()), "a cached result starts no scan");
        // `fresh` returns the previous result while the new scan runs.
        let v: serde_json::Value = serde_json::from_str(&cache.request(true, slow_fake_scan)).unwrap();
        assert_eq!((v["scanning"].clone(), v["scanned_at"].clone()), (true.into(), 1.into()));
        wait_until_idle(&cache);
        assert_eq!(SCANS.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn json_shape_has_the_documented_fields() {
        let s = fake_storage();
        let v: serde_json::Value = serde_json::from_str(&to_json(Some(&s), false, false)).unwrap();
        for k in ["scanning", "scanned_at", "scan_ms", "complete", "not_reached", "capacity", "categories", "items", "snapshots"] { assert!(v.get(k).is_some(), "{k}"); }
        assert_eq!(v["capacity"]["purgeable"]["unavailable"], "statfs failed");
    }
}
