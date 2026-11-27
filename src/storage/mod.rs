//! Where the disk space went: a bounded scan of a fixed list of locations on the boot volume, a
//! label for each item from a rules table, and the capacity split into used, purgeable and free.
//! Read-only: nothing here removes or moves a file. The scan reads metadata only (`lstat`), never
//! follows a symlink, never crosses a device and never walks the whole disk.
mod cache;
pub mod forecast;
pub mod trash;
#[cfg(test)]
mod converge_tests;
mod results;
mod rules;
mod volume;
mod walk;

pub use cache::{to_json, Cache, ScanFn};
pub use rules::{Label, Rule};
pub use volume::boot_capacity_reply;

/// (used, purgeable, free) bytes for the daily sample, read now without a scan. None when used or free
/// cannot be read; an unreadable purgeable share counts as 0 because `used` then already includes it.
pub fn capacity_triple() -> Option<(u64, u64, u64)> {
    triple_of(&volume::split_capacity(crate::iokit::boot_volume_capacity(), volume::available_and_important()))
}

fn triple_of(c: &Capacity) -> Option<(u64, u64, u64)> {
    match (&c.used, &c.free) {
        (Reading::Value(u), Reading::Value(f)) => Some((*u, if let Reading::Value(p) = c.purgeable { p } else { 0 }, *f)),
        _ => None,
    }
}

use crate::sampler::Reading;
use serde::Serialize;
use results::{Entry, Fresh, Results};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use volume::{Capacity, Snapshots};
use walk::Limits;

/// Total time for one `monitor-lizard storage` run, `tmutil` included. A slow scan would make the
/// monitor its own top consumer.
const TIME_BUDGET: Duration = Duration::from_secs(20);
/// Kept back for joining `tmutil`, building the reply and printing it.
const RESERVE: Duration = Duration::from_millis(750);
const MAX_ENTRIES_PER_LOCATION: u64 = 200_000;
/// Name of the saved result map in the data folder.
const STORE_FILE: &str = "storage-results.json";
/// How many of the largest unchanged locations take part in the round-robin refresh, and how many of
/// them one scan walks again. The rest are reused until their own modification time changes.
const REFRESH_POOL: usize = 8;
const REFRESH_PER_SCAN: usize = 2;
/// An unchanged location is not walked again for a refresh until it is this old (seconds), so scans
/// run back to back reuse everything.
const REFRESH_MIN_AGE_SECS: u64 = 600;

/// Why a size could not be read. The text is shown to the user.
pub mod reason {
    pub const PERMISSION_DENIED: &str = "permission denied";
    pub const UNREADABLE: &str = "could not be read";
    pub const STATFS_FAILED: &str = "statfs failed";
    pub const VOLUME_KEYS_UNREADABLE: &str = "volume capacity keys unreadable";
    pub const TMUTIL_NOT_FOUND: &str = "tmutil not found";
    pub const TMUTIL_COULD_NOT_START: &str = "tmutil could not start";
    pub const TMUTIL_TIMED_OUT: &str = "tmutil timed out";
    pub const TMUTIL_FAILED: &str = "tmutil failed";
    pub const HOME_UNKNOWN: &str = "HOME is not set to an absolute path, so no folder can be scanned";
    pub const HOME_UNTRUSTED: &str = "HOME is not a folder of yours at least two levels below the root (such as /Users/you), so nothing was scanned or moved";
    pub const SNAPSHOT_SIZE: &str = "no tool reports a snapshot size without root";
}

/// Said in every reply, because two facts change how the totals read.
const NOTES: &[&str] = &[
    "Sizes are allocated bytes (st_blocks * 512). Copy-on-write clones (cp -c) are counted once per copy, so a total can be larger than the space they really use.",
    "A category with lower_bound true is at least this large: some of its locations were not reached, were partial or could not be read.",
];

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Category { Developer, Caches, Logs, Downloads, Mail, Applications }

#[derive(Serialize, Clone, Debug)]
pub struct Item {
    /// Stable for a path across scans: the same id the removal route will take.
    pub id: String,
    pub path: String,
    pub category: Category,
    pub label: Label,
    pub rule: Rule,
    /// Allocated bytes, or why they could not be read.
    pub size: Reading<u64>,
    /// Bytes the files claim to hold; larger than `size` for a sparse file. Absent when `size` is.
    pub apparent_bytes: Option<u64>,
    pub entries: u64,
    /// False when the entry cap, a time limit or an unreadable subfolder stopped this item short, so
    /// `size` is a lower bound.
    pub complete: bool,
    pub unreadable: u32,
    /// Unix seconds when this size was measured. Older than `Storage::scanned_at` for a reused row.
    pub measured_at: u64,
    /// True when this scan did not walk the location and the size comes from an earlier scan.
    pub reused: bool,
    /// True when an earlier complete size is kept because a later measurement was partial or failed.
    pub stale: bool,
    /// The item's own modification time in nanoseconds when it was measured, so a removal can tell
    /// that it changed afterwards. Not sent to the page.
    #[serde(skip)]
    pub modified_ns: Option<u64>,
}

#[derive(Serialize, Debug)]
pub struct CategoryTotal {
    pub category: Category,
    pub bytes: u64,
    pub items: usize,
    pub unavailable_items: usize,
    /// Locations here that were not reached or only partly measured.
    pub unmeasured: usize,
    /// True when `bytes` is at least the real size rather than the size.
    pub lower_bound: bool,
}

#[derive(Serialize, Debug)]
pub struct Coverage { pub locations_total: usize, pub locations_measured: usize, pub bytes_measured: u64 }

#[derive(Serialize, Debug)]
pub struct Progress { pub done: usize, pub total: usize }

#[derive(Serialize, Debug)]
pub struct Storage {
    pub scanned_at: u64,
    pub scan_ms: u64,
    /// True only when every location was reached, fully readable and measured to the end. While false
    /// no total is final.
    pub complete: bool,
    /// Why no scan was made at all (the home folder is unknown), or null.
    pub refused: Option<&'static str>,
    /// Locations the time budget ended the scan before reaching.
    pub not_reached: Vec<String>,
    pub coverage: Coverage,
    pub progress: Progress,
    pub capacity: Capacity,
    pub categories: Vec<CategoryTotal>,
    pub items: Vec<Item>,
    pub snapshots: Snapshots,
    pub notes: &'static [&'static str],
}

/// FNV-1a: unlike the standard hasher it is specified, so an id stays the same across builds.
pub(crate) fn item_id(path: &str) -> String {
    let hash = path.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("{hash:016x}")
}

/// The most one location may take: a quarter of what is left.
fn slice(remaining: Duration) -> Duration { remaining / 4 }

struct Outcome { items: Vec<Item>, missed: Vec<(String, Category)> }

impl Outcome {
    /// True only when every location was reached, readable and measured to the end. Derived from the
    /// same counts as `coverage`, so the two can never disagree.
    fn complete(&self) -> bool {
        let (c, _) = coverage(&self.items, &self.missed);
        c.locations_measured == c.locations_total
    }
}

/// One location to measure, before the order is known.
struct Candidate { path: PathBuf, category: Category, project_idle: bool }

/// Where a location stands against what earlier scans learned. The derived order is the visit order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Rank {
    /// Never measured: walked first, in the fixed order.
    Never,
    /// Incomplete before, changed since, or due for its round-robin refresh: walked next, largest first.
    Stale,
    /// Complete and unchanged at the top level: reused without a walk.
    Fresh,
}

struct Scan<'a> { home: &'a Path, deadline: Instant, items: Vec<Item>, missed: Vec<(String, Category)>, max_entries: u64, now: u64 }

impl Scan<'_> {
    fn item(&self, c: &Candidate, text: &str, size: Reading<u64>, e: Option<&Entry>, reused: bool) -> Item {
        let v = rules::label_item(self.home, &c.path, c.project_idle);
        Item {
            id: item_id(text), path: text.to_string(), category: c.category, label: v.label, rule: v.rule, size,
            apparent_bytes: e.and_then(|e| e.apparent), entries: e.map_or(0, |e| e.entries), complete: e.is_some_and(|e| e.complete), unreadable: e.map_or(0, |e| e.unreadable),
            measured_at: e.map_or(self.now, |e| e.measured_at), reused, stale: e.is_some_and(|e| e.stale), modified_ns: trash::modified_ns(&c.path),
        }
    }

    fn push_entry(&mut self, c: &Candidate, text: &str, e: &Entry, reused: bool) {
        let item = self.item(c, text, Reading::Value(e.bytes), Some(e), reused);
        self.items.push(item);
    }

    /// Sizes one location with at most a quarter of the time that is left, so one huge folder cannot
    /// use up the scan, and merges the result into what is known.
    fn visit(&mut self, c: &Candidate, rank: Rank, results: &mut Results) {
        let text = c.path.display().to_string();
        let hint = trash::modified_ns(&c.path);
        let prior = results.get(&text).cloned();
        let confirmed = |e: &Entry| e.complete && e.mtime_hint.is_some() && e.mtime_hint == hint;
        if rank == Rank::Fresh {
            if let Some(e) = prior { self.push_entry(c, &text, &e, true); }
            return;
        }
        let now = Instant::now();
        if now >= self.deadline {
            // Out of time: what an earlier scan knows is shown (marked stale unless the top level is
            // unchanged), and only a location never measured is "not reached".
            match prior {
                Some(e) if confirmed(&e) => self.push_entry(c, &text, &e, true),
                Some(_) => { if let Some(e) = results.mark_stale(&text) { self.push_entry(c, &text, &e, true); } }
                None => self.missed.push((text, c.category)),
            }
            return;
        }
        let lim = Limits { deadline: now + slice(self.deadline - now), max_entries: self.max_entries };
        match walk::walk(&c.path, lim) {
            None => results.forget(&text),
            Some(Ok(w)) => {
                // A skipped subfolder makes the size too small, so it is as partial as a cut-off walk.
                let fresh = Fresh { bytes: w.allocated, apparent: Some(w.apparent), entries: w.entries, unreadable: w.unreadable, complete: w.stop.is_none() && w.unreadable == 0, mtime_hint: hint };
                let e = results.merge(&text, fresh, self.now);
                let reused = e.measured_at != self.now;
                self.push_entry(c, &text, &e, reused);
            }
            Some(Err(why)) => match results.mark_stale(&text) {
                Some(e) => self.push_entry(c, &text, &e, true),
                None => { let item = self.item(c, &text, Reading::Unavailable(why), None, false); self.items.push(item); }
            },
        }
    }
}

/// Every location this scan may measure, in the fixed order of usefulness per second spent. It is
/// also the visit order on a first run, and it matters because the budget usually runs out:
/// Xcode's DerivedData and the Docker folder are large and safe to act on; app caches come next,
/// biggest first by a cheap one-level size; simulators and logs follow; the `node_modules` search is
/// slow and comes after them; the user's own folders and the apps in `/Applications` cannot be
/// `safe` and go last. Each app bundle is its own location with its own entry cap.
fn discover(home: &Path, applications: &Path, deadline: Instant, now: u64, max_entries: u64) -> (Vec<Candidate>, Vec<(String, Category)>) {
    let (mut out, mut missed) = (Vec::new(), Vec::new());
    let add = |out: &mut Vec<Candidate>, path: PathBuf, category, project_idle| out.push(Candidate { path, category, project_idle });
    let lib = |rel: &str| home.join("Library").join(rel);
    add(&mut out, lib("Developer/Xcode/DerivedData"), Category::Developer, false);
    add(&mut out, lib("Containers/com.docker.docker"), Category::Developer, false);
    for child in caches_by_size(&lib("Caches"), Instant::now() + slice(deadline.saturating_duration_since(Instant::now()))) { add(&mut out, child, Category::Caches, false); }
    add(&mut out, lib("Developer/CoreSimulator"), Category::Developer, false);
    add(&mut out, lib("Logs"), Category::Logs, false);
    if Instant::now() >= deadline {
        missed.push(("node_modules search".into(), Category::Developer));
    } else {
        let limit = Limits { deadline: Instant::now() + slice(deadline.saturating_duration_since(Instant::now())), max_entries };
        let (modules, cut) = walk::find_node_modules(home, limit, now);
        if cut { missed.push(("node_modules search (cut short)".into(), Category::Developer)); }
        for (path, idle) in modules { add(&mut out, path, Category::Developer, idle); }
    }
    add(&mut out, home.join("Downloads"), Category::Downloads, false);
    add(&mut out, lib("Mail"), Category::Mail, false);
    for bundle in app_bundles(applications) { add(&mut out, bundle, Category::Applications, false); }
    (out, missed)
}

/// The entries of the applications folder, by name. A folder that cannot be listed is returned
/// itself, so the reason shows up as one unavailable item; a missing one has no entries.
fn app_bundles(dir: &Path) -> Vec<PathBuf> {
    match std::fs::read_dir(dir) {
        Ok(rd) => { let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect(); v.sort(); v }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => vec![dir.to_path_buf()],
    }
}

/// The order to visit `cands` in, with each one's rank. Never-measured locations come first, in the
/// fixed order; then stale ones, largest known size first; then unchanged ones, which cost nothing.
/// Of the largest unchanged locations, the two measured longest ago (and at least ten minutes ago) are
/// walked again each scan, so a
/// change deep inside a folder whose own time did not move is still found, in turn.
fn visit_order(cands: &[Candidate], results: &Results, now: u64) -> Vec<(usize, Rank)> {
    // (index, rank, last known size, measured_at)
    let mut ranked: Vec<(usize, Rank, u64, u64)> = cands.iter().enumerate().map(|(i, c)| match results.get(&c.path.display().to_string()) {
        None => (i, Rank::Never, 0, 0),
        Some(e) => {
            let unchanged = e.complete && e.mtime_hint.is_some() && e.mtime_hint == trash::modified_ns(&c.path);
            (i, if unchanged { Rank::Fresh } else { Rank::Stale }, e.bytes, e.measured_at)
        }
    }).collect();
    let mut pool: Vec<usize> = (0..ranked.len()).filter(|&k| ranked[k].1 == Rank::Fresh).collect();
    pool.sort_by(|&a, &b| ranked[b].2.cmp(&ranked[a].2).then(ranked[a].0.cmp(&ranked[b].0)));
    pool.truncate(REFRESH_POOL);
    pool.sort_by_key(|&k| (ranked[k].3, ranked[k].0));
    let due: Vec<usize> = pool.into_iter().filter(|&k| now.saturating_sub(ranked[k].3) >= REFRESH_MIN_AGE_SECS).take(REFRESH_PER_SCAN).collect();
    for k in due { ranked[k].1 = Rank::Stale; }
    ranked.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| if a.1 == Rank::Never { a.0.cmp(&b.0) } else { b.2.cmp(&a.2).then(a.0.cmp(&b.0)) }));
    ranked.into_iter().map(|(i, r, _, _)| (i, r)).collect()
}

fn scan_items_with(home: &Path, applications: &Path, deadline: Instant, now: u64, results: &mut Results, max_entries: u64) -> Outcome {
    let (cands, missed) = discover(home, applications, deadline, now, max_entries);
    let order = visit_order(&cands, results, now);
    let mut s = Scan { home, deadline, items: Vec::new(), missed, max_entries, now };
    let mut seen = HashSet::new();
    for (i, rank) in order {
        seen.insert(cands[i].path.display().to_string());
        s.visit(&cands[i], rank, results);
    }
    results.prune_missing(&seen);
    Outcome { items: s.items, missed: s.missed }
}

/// Everything except capacity and snapshots, which describe the volume rather than a folder.
fn scan_items(home: &Path, applications: &Path, deadline: Instant, now: u64, results: &mut Results) -> Outcome {
    scan_items_with(home, applications, deadline, now, results, MAX_ENTRIES_PER_LOCATION)
}

/// The children of the caches folder, largest first. When the folder cannot be listed the folder
/// itself is returned, so the reason shows up as one unavailable item.
fn caches_by_size(dir: &Path, deadline: Instant) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![dir.to_path_buf()] };
    let mut sized: Vec<(u64, PathBuf)> = Vec::new();
    for e in rd.flatten() {
        let bytes = if Instant::now() < deadline { walk::one_level_bytes(&e.path(), deadline) } else { 0 };
        sized.push((bytes, e.path()));
    }
    sized.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    sized.into_iter().map(|(_, p)| p).collect()
}

/// The total for `c`, added to the list when it is the first location seen in that category.
fn total_for(out: &mut Vec<CategoryTotal>, c: Category) -> &mut CategoryTotal {
    let at = out.iter().position(|t| t.category == c).unwrap_or_else(|| {
        out.push(CategoryTotal { category: c, bytes: 0, items: 0, unavailable_items: 0, unmeasured: 0, lower_bound: false });
        out.len() - 1
    });
    &mut out[at]
}

fn categories(items: &[Item], missed: &[(String, Category)]) -> Vec<CategoryTotal> {
    let mut out: Vec<CategoryTotal> = Vec::new();
    for i in items {
        let t = total_for(&mut out, i.category);
        t.items += 1;
        match i.size {
            Reading::Value(b) => { t.bytes += b; if !i.complete { t.unmeasured += 1; } }
            _ => t.unavailable_items += 1,
        }
    }
    for (_, c) in missed { total_for(&mut out, *c).unmeasured += 1; }
    for t in &mut out { t.lower_bound = t.unmeasured > 0 || t.unavailable_items > 0; }
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    out
}

fn coverage(items: &[Item], missed: &[(String, Category)]) -> (Coverage, Progress) {
    let bytes_measured = items.iter().filter_map(|i| match i.size { Reading::Value(b) => Some(b), _ => None }).sum();
    let measured = items.iter().filter(|i| i.complete && matches!(i.size, Reading::Value(_))).count();
    let total = items.len() + missed.len();
    (Coverage { locations_total: total, locations_measured: measured, bytes_measured }, Progress { done: items.len(), total })
}

#[cfg(target_os = "macos")]
extern "C" {
    // In libSystem; the `libc` crate does not declare it.
    fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
}

/// `QOS_CLASS_UTILITY` from `<sys/qos.h>`.
#[cfg(target_os = "macos")]
const QOS_UTILITY: u32 = 0x11;

/// Lowers this thread's CPU and disk priority so the scan yields to the user's work. The utility
/// class, not the background one: the background class throttles disk reads so hard that a scan
/// reached about 5,000 entries in 27 s on a developer home.
#[cfg(target_os = "macos")]
fn run_in_background() {
    static LOGGED: std::sync::Once = std::sync::Once::new();
    // SAFETY: sets the QoS class of the calling thread only; no pointers are involved.
    let rc = unsafe { pthread_set_qos_class_self_np(QOS_UTILITY, 0) };
    if rc != 0 { LOGGED.call_once(|| eprintln!("monitor-lizard: could not lower the storage scan's priority (error {rc}); it runs at normal priority")); }
}

#[cfg(not(target_os = "macos"))]
fn run_in_background() {}

/// The home folder, canonicalised, or why it cannot be used. Removal treats everything under it as the
/// user's own, so `HOME` must name the same folder as the account database does for the current user
/// (`HOME=/Users/you/Library` is refused), be a real folder of that user, and sit at least two levels
/// below the root (never `/` or `/Users`). An unset or relative `HOME` would make every location
/// resolve against the current directory.
pub(crate) fn home_or_reason() -> Result<PathBuf, &'static str> {
    let uid = current_uid();
    home_from(std::env::var_os("HOME"), uid, account_home(uid))
}

fn home_from(env_home: Option<std::ffi::OsString>, uid: u32, account: Option<PathBuf>) -> Result<PathBuf, &'static str> {
    let candidate = PathBuf::from(env_home.ok_or(reason::HOME_UNKNOWN)?);
    if !candidate.is_absolute() { return Err(reason::HOME_UNKNOWN); }
    trusted_home(&candidate, uid, account.as_deref()).ok_or(reason::HOME_UNTRUSTED)
}

pub(crate) fn current_uid() -> u32 {
    // SAFETY: geteuid has no arguments and cannot fail.
    unsafe { libc::geteuid() }
}

/// The home folder the account database records for `uid`.
fn account_home(uid: u32) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let mut buf = vec![0u8; 4096];
    // SAFETY: `pwd` and `buf` outlive the call, `buf.len()` is its true size, and `pw_dir` is only read
    // when the call reports a match, as a NUL-terminated string inside `buf`.
    unsafe {
        let mut pwd: libc::passwd = std::mem::zeroed();
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        if libc::getpwuid_r(uid, &mut pwd, buf.as_mut_ptr().cast(), buf.len(), &mut found) != 0 || found.is_null() || pwd.pw_dir.is_null() { return None; }
        Some(PathBuf::from(std::ffi::OsStr::from_bytes(std::ffi::CStr::from_ptr(pwd.pw_dir).to_bytes())))
    }
}

/// `candidate` as its canonical path when it is a folder of `uid`, two levels down, and (when the
/// account's folder is known) the same folder as that.
fn trusted_home(candidate: &Path, uid: u32, account: Option<&Path>) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let canonical = candidate.canonicalize().ok()?;
    let depth = |p: &Path| p.components().filter(|c| matches!(c, std::path::Component::Normal(_))).count();
    // Both spellings count: `/Users/../` and a link to `/` must not pass.
    if depth(candidate) < 2 || depth(&canonical) < 2 { return None; }
    if account? .canonicalize().ok()? != canonical { return None; }
    let meta = std::fs::metadata(&canonical).ok()?;
    (meta.is_dir() && meta.uid() == uid).then_some(canonical)
}

fn now_secs() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

/// One full scan for the command line, `tmutil` included, within `TIME_BUDGET`. Blocking: call it off
/// the request thread.
pub fn scan() -> Storage { scan_stored(TIME_BUDGET) }

/// The server's background scan: the same scan with the longer budget, so repeated runs converge.
pub fn scan_in_background() -> Storage { scan_stored(cache::SERVER_TIME_BUDGET) }

/// Runs a scan against the result map kept for this process and saved in the data folder, so each
/// run starts from what the last one learned. Scans never overlap (the cache starts one at a time),
/// and the lock only guards the command line against a second caller.
fn scan_stored(budget: Duration) -> Storage {
    static RESULTS: Mutex<Option<Results>> = Mutex::new(None);
    let home = home_or_reason();
    let store = home.is_ok().then(|| crate::paths::data_dir().join(STORE_FILE));
    let mut guard = RESULTS.lock().unwrap_or_else(|e| e.into_inner());
    let results = guard.get_or_insert_with(|| store.as_deref().map(Results::load).unwrap_or_default());
    let out = scan_for(home, budget, results);
    if let Some(p) = store.as_deref().filter(|_| out.refused.is_none()) {
        if let Err(e) = results.save(p) { eprintln!("monitor-lizard: could not save the storage results: {e}"); }
    }
    out
}

/// `scan` for a given home, so a refused home can be tested without changing the environment.
fn scan_for(home: Result<PathBuf, &'static str>, budget: Duration, results: &mut Results) -> Storage {
    run_in_background();
    walk::pace_reset();
    let started = Instant::now();
    // `tmutil` runs beside the walk instead of after it, so it adds no wall time.
    let tmutil = std::thread::spawn(volume::local_snapshots);
    let (o, refused) = match home {
        Ok(h) => (scan_items(&h, Path::new("/Applications"), started + budget - RESERVE, now_secs(), results), None),
        Err(why) => (Outcome { items: Vec::new(), missed: Vec::new() }, Some(why)),
    };
    let capacity = volume::split_capacity(crate::iokit::boot_volume_capacity(), volume::available_and_important());
    let snapshots = tmutil.join().unwrap_or_else(|_| volume::snapshots_from(Err(reason::TMUTIL_FAILED)));
    let (coverage, progress) = coverage(&o.items, &o.missed);
    Storage {
        scanned_at: now_secs(), scan_ms: started.elapsed().as_millis() as u64, complete: refused.is_none() && o.complete(), refused,
        not_reached: o.missed.iter().map(|(p, _)| p.clone()).collect(), coverage, progress, capacity,
        categories: categories(&o.items, &o.missed), items: o.items, snapshots, notes: NOTES,
    }
}

#[cfg(test)]
impl Storage {
    /// A small result for tests of the cache and the JSON shape.
    pub(crate) fn fake(scanned_at: u64) -> Storage {
        Storage {
            scanned_at, scan_ms: 1, complete: true, refused: None, not_reached: vec![], coverage: Coverage { locations_total: 0, locations_measured: 0, bytes_measured: 0 },
            progress: Progress { done: 0, total: 0 }, capacity: volume::split_capacity(None, None), categories: vec![], items: vec![],
            snapshots: volume::snapshots_from(Err(reason::TMUTIL_NOT_FOUND)), notes: NOTES,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    pub(super) fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ml-storage-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn in_secs(s: u64) -> Instant { Instant::now() + Duration::from_secs(s) }

    fn is_root() -> bool {
        // SAFETY: `geteuid` takes no arguments and cannot fail.
        unsafe { libc::geteuid() == 0 }
    }

    /// Denied-folder tests use mode 000, which root ignores.
    fn skip_as_root(test: &str) -> bool {
        if is_root() { eprintln!("skipped {test}: running as root, where mode 000 does not deny access"); }
        is_root()
    }

    fn by_suffix<'a>(items: &'a [Item], suffix: &str) -> &'a Item { items.iter().find(|i| i.path.ends_with(suffix)).unwrap_or_else(|| panic!("no item ending {suffix}")) }

    #[test]
    fn an_expired_budget_lists_the_locations_not_reached_and_marks_the_result_partial() {
        let home = temp("expired");
        fs::create_dir_all(home.join("Downloads")).unwrap();
        let o = scan_items(&home, &home.join("none"), Instant::now() - Duration::from_secs(1), now_secs(), &mut Results::default());
        assert!(o.items.is_empty() && !o.complete());
        assert!(o.missed.iter().any(|(p, c)| p.ends_with("Downloads") && *c == Category::Downloads));
        assert!(o.missed.iter().any(|(p, _)| p == "node_modules search"));
        let (c, p) = coverage(&o.items, &o.missed);
        assert_eq!((c.locations_measured, c.bytes_measured, p.done), (0, 0, 0));
        assert_eq!(c.locations_total, o.missed.len());
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_missing_location_is_omitted_and_a_denied_one_is_listed() {
        if skip_as_root("a_missing_location_is_omitted_and_a_denied_one_is_listed") { return; }
        let home = temp("omit");
        let locked = home.join("Library/Mail");
        fs::create_dir_all(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let o = scan_items(&home, &home.join("none"), in_secs(30), now_secs(), &mut Results::default());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!o.items.iter().any(|i| i.path.contains("docker") || i.path.contains("DerivedData")), "missing locations are omitted");
        let mail = by_suffix(&o.items, "Library/Mail");
        assert_eq!(mail.size, Reading::Unavailable(reason::PERMISSION_DENIED));
        assert_eq!((mail.label, mail.apparent_bytes), (Label::Yours, None));
        let c = categories(&o.items, &o.missed);
        let t = c.iter().find(|t| t.category == Category::Mail).unwrap();
        assert!(t.lower_bound && t.unavailable_items == 1, "an unreadable folder makes the total a lower bound");
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn caches_are_listed_per_app_largest_first_and_each_needs_review() {
        let home = temp("caches");
        fs::create_dir_all(home.join("Library/Caches/com.small")).unwrap();
        fs::create_dir_all(home.join("Library/Caches/com.big")).unwrap();
        fs::write(home.join("Library/Caches/com.big/f"), vec![1u8; 512 * 1024]).unwrap();
        fs::create_dir_all(home.join("Library/Caches/Homebrew")).unwrap();
        fs::write(home.join("Library/Caches/Homebrew/f"), vec![1u8; 4096]).unwrap();
        let o = scan_items(&home, &home.join("none"), in_secs(30), now_secs(), &mut Results::default());
        let order: Vec<_> = o.items.iter().filter(|i| i.category == Category::Caches).map(|i| i.path.rsplit('/').next().unwrap().to_string()).collect();
        assert_eq!(order, ["com.big", "Homebrew", "com.small"]);
        assert_eq!(by_suffix(&o.items, "com.big").label, Label::Review);
        assert_eq!(by_suffix(&o.items, "Homebrew").label, Label::Safe);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn locations_are_measured_in_the_documented_order() {
        let home = temp("order");
        for rel in ["Library/Developer/Xcode/DerivedData", "Library/Containers/com.docker.docker", "Library/Caches/a", "Library/Developer/CoreSimulator", "Library/Logs", "Downloads", "Library/Mail", "apps/none/Some.app"] {
            fs::create_dir_all(home.join(rel)).unwrap();
        }
        fs::create_dir_all(home.join("proj/node_modules")).unwrap();
        let o = scan_items(&home, &home.join("apps/none"), in_secs(30), now_secs(), &mut Results::default());
        let tails: Vec<_> = o.items.iter().map(|i| i.path.strip_prefix(&home.display().to_string()).unwrap().to_string()).collect();
        assert_eq!(tails, ["/Library/Developer/Xcode/DerivedData", "/Library/Containers/com.docker.docker", "/Library/Caches/a", "/Library/Developer/CoreSimulator", "/Library/Logs", "/proj/node_modules", "/Downloads", "/Library/Mail", "/apps/none/Some.app"]);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn one_location_gets_at_most_a_quarter_of_the_time_left() {
        assert_eq!(slice(Duration::from_secs(20)), Duration::from_secs(5));
        let mut left = Duration::from_secs(20);
        for _ in 0..3 {
            let s = slice(left);
            assert!(s * 4 <= left);
            left -= s;
        }
        assert!(left > Duration::from_secs(8), "three huge locations in a row still leave a share for the rest: {left:?}");
    }

    #[test]
    fn a_location_that_hits_its_slice_is_partial_and_lowers_the_category_bound() {
        let home = temp("slice");
        let big = home.join("Library/Developer/Xcode/DerivedData");
        fs::create_dir_all(&big).unwrap();
        for i in 0..4000 { fs::write(big.join(format!("f{i}")), b"x").unwrap(); }
        // 40 ms left gives this location 10 ms, less than the walk needs: it is cut off by its slice,
        // not by the whole deadline, which is still in the future.
        let deadline = Instant::now() + Duration::from_millis(40);
        let mut s = Scan { home: &home, deadline, items: vec![], missed: vec![], max_entries: MAX_ENTRIES_PER_LOCATION, now: now_secs() };
        s.visit(&Candidate { path: big.clone(), category: Category::Developer, project_idle: false }, Rank::Never, &mut Results::default());
        assert_eq!(s.items.len(), 1);
        assert!(!s.items[0].complete, "cut off by its slice");
        assert!(s.items[0].entries < 4000 && matches!(s.items[0].size, Reading::Value(_)));
        let o = Outcome { items: s.items, missed: s.missed };
        assert!(!o.complete());
        assert!(categories(&o.items, &o.missed).iter().any(|t| t.category == Category::Developer && t.lower_bound && t.unmeasured == 1));
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_subfolder_that_cannot_be_read_makes_the_location_partial_and_the_total_a_lower_bound() {
        if skip_as_root("a_subfolder_that_cannot_be_read_makes_the_location_partial_and_the_total_a_lower_bound") { return; }
        let home = temp("unreadable");
        let app = home.join("Library/Caches/com.x");
        fs::create_dir_all(app.join("locked")).unwrap();
        fs::write(app.join("ok"), vec![1u8; 64 * 1024]).unwrap();
        fs::set_permissions(app.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
        let o = scan_items(&home, &home.join("none"), in_secs(30), now_secs(), &mut Results::default());
        fs::set_permissions(app.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
        let item = by_suffix(&o.items, "com.x");
        assert!(matches!(item.size, Reading::Value(_)) && item.unreadable == 1);
        assert!(!item.complete, "a skipped subfolder is not a complete size");
        assert!(categories(&o.items, &o.missed).iter().find(|t| t.category == Category::Caches).unwrap().lower_bound);
        assert!(!o.complete());
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_denied_location_makes_the_whole_scan_incomplete() {
        if skip_as_root("a_denied_location_makes_the_whole_scan_incomplete") { return; }
        let home = temp("denied-location");
        fs::create_dir_all(home.join("Downloads")).unwrap();
        fs::create_dir_all(home.join("Library/Mail")).unwrap();
        fs::set_permissions(home.join("Library/Mail"), fs::Permissions::from_mode(0o000)).unwrap();
        let o = scan_items(&home, &home.join("none"), in_secs(30), now_secs(), &mut Results::default());
        fs::set_permissions(home.join("Library/Mail"), fs::Permissions::from_mode(0o755)).unwrap();
        let (c, _) = coverage(&o.items, &o.missed);
        assert!(c.locations_measured < c.locations_total);
        assert!(!o.complete(), "complete follows coverage");
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_fully_readable_tree_is_complete_at_every_level() {
        let home = temp("complete");
        fs::create_dir_all(home.join("Library/Caches/com.x/sub")).unwrap();
        fs::write(home.join("Library/Caches/com.x/sub/f"), b"x").unwrap();
        fs::create_dir_all(home.join("Downloads")).unwrap();
        let o = scan_items(&home, &home.join("none"), in_secs(30), now_secs(), &mut Results::default());
        assert!(o.complete() && o.items.iter().all(|i| i.complete));
        let c = categories(&o.items, &o.missed);
        assert!(c.iter().all(|t| !t.lower_bound && t.unmeasured == 0));
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn without_a_usable_home_nothing_is_scanned_and_the_reason_is_given() {
        for why in [reason::HOME_UNKNOWN, reason::HOME_UNTRUSTED] {
            let s = scan_for(Err(why), TIME_BUDGET, &mut Results::default());
            assert!(s.items.is_empty() && !s.complete, "{why}");
            assert_eq!(s.refused, Some(why));
            let v: serde_json::Value = serde_json::from_str(&to_json(Some(&s), false, false)).unwrap();
            assert_eq!(v["refused"].as_str(), Some(why));
        }
    }

    #[test]
    fn node_modules_are_found_only_in_project_folders_and_never_in_user_content() {
        let home = temp("projects");
        for rel in ["proj/node_modules/pkg/node_modules", "Developer/a/b/c/node_modules", "Documents/p/node_modules", "Desktop/node_modules", "Music/p/node_modules", ".hidden/node_modules", "Library/x/node_modules", "work/git/r/node_modules", "other/deep/deeper/node_modules", "node_modules"] {
            fs::create_dir_all(home.join(rel)).unwrap();
        }
        let (found, cut) = walk::find_node_modules(&home, Limits { deadline: in_secs(30), max_entries: 100_000 }, now_secs());
        assert!(!cut);
        let mut tails: Vec<_> = found.iter().map(|(p, _)| p.strip_prefix(&home).unwrap().display().to_string()).collect();
        tails.sort();
        assert_eq!(tails, ["Developer/a/b/c/node_modules", "node_modules", "proj/node_modules", "work/git/r/node_modules"]);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn category_totals_sum_values_and_say_when_they_are_a_lower_bound() {
        let item = |category, size, complete| Item { id: "i".into(), path: "p".into(), category, label: Label::Review, rule: Rule::Unclassified, size, apparent_bytes: None, entries: 0, complete, unreadable: 0, measured_at: 0, reused: false, stale: false, modified_ns: None };
        let items = [item(Category::Logs, Reading::Value(5), true), item(Category::Logs, Reading::Value(7), true), item(Category::Mail, Reading::Unavailable(reason::PERMISSION_DENIED), false), item(Category::Caches, Reading::Value(3), false)];
        let missed = [("x".to_string(), Category::Downloads)];
        let c = categories(&items, &missed);
        let get = |cat| c.iter().find(|t| t.category == cat).unwrap();
        assert_eq!((get(Category::Logs).bytes, get(Category::Logs).items, get(Category::Logs).lower_bound, get(Category::Logs).unmeasured), (12, 2, false, 0));
        assert_eq!((get(Category::Mail).bytes, get(Category::Mail).unavailable_items, get(Category::Mail).lower_bound), (0, 1, true));
        assert_eq!((get(Category::Caches).unmeasured, get(Category::Caches).lower_bound), (1, true), "a partial item");
        assert_eq!((get(Category::Downloads).items, get(Category::Downloads).unmeasured, get(Category::Downloads).lower_bound), (0, 1, true), "a location never reached");
        let (cov, prog) = coverage(&items, &missed);
        assert_eq!((cov.locations_total, cov.locations_measured, cov.bytes_measured, prog.done, prog.total), (5, 2, 15, 4, 5));
    }

    #[test]
    fn item_ids_are_stable_across_scans_and_differ_by_path() {
        let home = temp("ids");
        fs::create_dir_all(home.join("Library/Logs")).unwrap();
        fs::create_dir_all(home.join("Downloads")).unwrap();
        let ids = |o: &Outcome| o.items.iter().map(|i| (i.path.clone(), i.id.clone())).collect::<Vec<_>>();
        let a = ids(&scan_items(&home, &home.join("none"), in_secs(30), now_secs(), &mut Results::default()));
        fs::write(home.join("Downloads/new"), b"x").unwrap();
        let b = ids(&scan_items(&home, &home.join("none"), in_secs(30), now_secs(), &mut Results::default()));
        assert_eq!(a, b);
        assert_ne!(a[0].1, a[1].1);
        assert_eq!(item_id("/a"), "07d66707b49cd92d", "FNV-1a is specified, so this value must not change");
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn json_shape_has_the_documented_fields() {
        let v: serde_json::Value = serde_json::from_str(&to_json(Some(&Storage::fake(1)), false, false)).unwrap();
        for k in ["scanning", "scanned_at", "scan_ms", "complete", "not_reached", "refused", "coverage", "progress", "capacity", "categories", "items", "snapshots", "notes"] { assert!(v.get(k).is_some(), "{k}"); }
        assert_eq!(v["capacity"]["purgeable"]["unavailable"], reason::STATFS_FAILED);
        assert!(v["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("clones")));
    }

    #[test]
    fn a_real_scan_with_an_expired_budget_finishes_at_once_and_says_what_it_missed() {
        // Not `scan()` itself (it reads the real home); the same code path with a past deadline.
        let home = temp("quick");
        let t = Instant::now();
        let o = scan_items(&home, &home.join("none"), Instant::now() - Duration::from_secs(1), now_secs(), &mut Results::default());
        assert!(t.elapsed() < Duration::from_secs(2) && !o.complete());
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn only_the_accounts_own_home_is_trusted_and_it_is_canonicalised() {
        let me = current_uid();
        let d = temp("trusted").canonicalize().unwrap();
        let ok = |c: &Path, acct: &Path| trusted_home(c, me, Some(acct));
        assert_eq!(ok(&d, &d), Some(d.clone()));
        assert_eq!(ok(Path::new(&format!("{}/", d.display())), &d), Some(d.clone()), "a trailing slash is fine");
        assert!(trusted_home(&d, me.wrapping_add(1), Some(&d)).is_none(), "owned by someone else");
        assert!(trusted_home(&d, me, None).is_none(), "no account folder to compare with");
        fs::create_dir_all(d.join("Library")).unwrap();
        assert!(ok(&d.join("Library"), &d).is_none(), "HOME=<home>/Library is not the home");
        let other = temp("trusted-other");
        assert!(ok(&other, &d).is_none(), "HOME and the account disagree");
        // A symlinked HOME is accepted and comes back as the real folder, so items are not 'outside' it.
        let link = temp("trusted-links").join("home-link");
        std::os::unix::fs::symlink(&d, &link).unwrap();
        assert_eq!(ok(&link, &d), Some(d.clone()));
        for bad in ["/", "/Users", "/Users/", "/tmp", "/private", "/Users/.."] { assert!(ok(Path::new(bad), Path::new(bad)).is_none(), "{bad:?}"); }
        assert!(ok(&d.join("no-such-folder"), &d).is_none());
        let file = d.join("file");
        fs::write(&file, "x").unwrap();
        assert!(ok(&file, &file).is_none(), "a file is not a home");
        let to_root = temp("trusted-links").join("rootlink");
        std::os::unix::fs::symlink("/", &to_root).unwrap();
        assert!(ok(&to_root, &to_root).is_none());
    }

    #[test]
    fn home_from_says_why_it_refuses() {
        let me = current_uid();
        let d = temp("home-from").canonicalize().unwrap();
        assert_eq!(home_from(None, me, Some(d.clone())), Err(reason::HOME_UNKNOWN));
        for rel in ["", "relative/dir", "~"] { assert_eq!(home_from(Some(rel.into()), me, Some(d.clone())), Err(reason::HOME_UNKNOWN), "{rel:?}"); }
        assert_eq!(home_from(Some("/".into()), me, Some(d.clone())), Err(reason::HOME_UNTRUSTED));
        assert_eq!(home_from(Some(d.clone().into()), me, Some(d.clone())), Ok(d));
        assert!(account_home(me).is_some_and(|h| h.is_absolute()), "this user has an account folder");
    }
}
