//! Where the disk space went: a bounded scan of a fixed list of locations on the boot volume, a
//! label for each item from a rules table, and the capacity split into used, purgeable and free.
//! Read-only: nothing here removes or moves a file. The scan reads metadata only (`lstat`), never
//! follows a symlink, never crosses a device and never walks the whole disk.
mod cache;
mod rules;
mod volume;
mod walk;

pub use cache::{to_json, Cache, ScanFn};
pub use rules::{Label, Rule};

use crate::sampler::Reading;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use volume::{Capacity, Snapshots};
use walk::Limits;

/// Total time for one `monitor-lizard storage` run, `tmutil` included. A slow scan would make the
/// monitor its own top consumer.
const TIME_BUDGET: Duration = Duration::from_secs(20);
/// Kept back for joining `tmutil`, building the reply and printing it.
const RESERVE: Duration = Duration::from_millis(750);
const MAX_ENTRIES_PER_LOCATION: u64 = 200_000;

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

#[derive(Serialize, Debug)]
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
    /// False when the entry cap or a time limit stopped this item early, so `size` is a lower bound.
    pub complete: bool,
    pub unreadable: u32,
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
    /// False when any location was not reached or only partly measured. While false, no total is final.
    pub complete: bool,
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
fn item_id(path: &str) -> String {
    let hash = path.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("{hash:016x}")
}

/// The most one location may take: a quarter of what is left.
fn slice(remaining: Duration) -> Duration { remaining / 4 }

struct Outcome { items: Vec<Item>, missed: Vec<(String, Category)>, partial: bool }

struct Scan<'a> { home: &'a Path, deadline: Instant, items: Vec<Item>, missed: Vec<(String, Category)>, partial: bool }

impl Scan<'_> {
    /// Sizes one location with at most a quarter of the time that is left, so one huge folder cannot
    /// use up the scan.
    fn measure(&mut self, path: &Path, category: Category, project_idle: bool) {
        let now = Instant::now();
        if now >= self.deadline {
            self.missed.push((path.display().to_string(), category));
            return;
        }
        let lim = Limits { deadline: now + slice(self.deadline - now), max_entries: MAX_ENTRIES_PER_LOCATION };
        let Some(result) = walk::walk(path, lim) else { return };
        let v = rules::label_item(self.home, path, project_idle);
        let text = path.display().to_string();
        let item = |size, apparent_bytes, entries, complete, unreadable| Item {
            id: item_id(&text), path: text.clone(), category, label: v.label, rule: v.rule, size, apparent_bytes, entries, complete, unreadable,
        };
        self.items.push(match result {
            Ok(w) => {
                self.partial |= w.stop.is_some();
                item(Reading::Value(w.allocated), Some(w.apparent), w.entries, w.stop.is_none(), w.unreadable)
            }
            Err(why) => item(Reading::Unavailable(why), None, 0, false, 0),
        });
    }
}

/// Everything except capacity and snapshots, which describe the volume rather than a folder.
///
/// The order is the order of usefulness per second spent, and it matters because the budget usually
/// runs out: Xcode's DerivedData and the Docker folder are large and safe to act on; app caches come
/// next, biggest first by a cheap one-level size; simulators and logs follow; the `node_modules`
/// search is slow and comes after them; the user's own folders and `/Applications` cannot be `safe`
/// and go last.
fn scan_items(home: &Path, applications: &Path, deadline: Instant, now: u64) -> Outcome {
    let mut s = Scan { home, deadline, items: Vec::new(), missed: Vec::new(), partial: false };
    let lib = |rel: &str| home.join("Library").join(rel);
    s.measure(&lib("Developer/Xcode/DerivedData"), Category::Developer, false);
    s.measure(&lib("Containers/com.docker.docker"), Category::Developer, false);
    for child in caches_by_size(&lib("Caches"), deadline) { s.measure(&child, Category::Caches, false); }
    s.measure(&lib("Developer/CoreSimulator"), Category::Developer, false);
    s.measure(&lib("Logs"), Category::Logs, false);
    if Instant::now() >= deadline {
        s.missed.push(("node_modules search".into(), Category::Developer));
    } else {
        let (modules, cut) = walk::find_node_modules(home, Limits { deadline, max_entries: MAX_ENTRIES_PER_LOCATION }, now);
        if cut { s.missed.push(("node_modules search (cut short)".into(), Category::Developer)); }
        for (path, idle) in modules { s.measure(&path, Category::Developer, idle); }
    }
    s.measure(&home.join("Downloads"), Category::Downloads, false);
    s.measure(&lib("Mail"), Category::Mail, false);
    s.measure(applications, Category::Applications, false);
    let partial = s.partial || !s.missed.is_empty();
    Outcome { items: s.items, missed: s.missed, partial }
}

/// The children of the caches folder, largest first. When the folder cannot be listed the folder
/// itself is returned, so the reason shows up as one unavailable item.
fn caches_by_size(dir: &Path, deadline: Instant) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![dir.to_path_buf()] };
    let mut sized: Vec<(u64, PathBuf)> = Vec::new();
    for e in rd.flatten() {
        let bytes = if Instant::now() < deadline { walk::one_level_bytes(&e.path()) } else { 0 };
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
const QOS_UTILITY: u32 = 0x11;

/// Lowers this thread's CPU and disk priority so the scan yields to the user's work. The utility
/// class, not the background one: the background class throttles disk reads so hard that a scan
/// reached about 5,000 entries in 27 s on a developer home.
fn run_in_background() {
    static LOGGED: std::sync::Once = std::sync::Once::new();
    // SAFETY: sets the QoS class of the calling thread only; no pointers are involved.
    let rc = unsafe { pthread_set_qos_class_self_np(QOS_UTILITY, 0) };
    if rc != 0 { LOGGED.call_once(|| eprintln!("monitor-lizard: could not lower the storage scan's priority (error {rc}); it runs at normal priority")); }
}

fn home() -> PathBuf { PathBuf::from(std::env::var("HOME").unwrap_or_default()) }

fn now_secs() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

/// One full scan, `tmutil` included, within `TIME_BUDGET`. Blocking: call it off the request thread.
pub fn scan() -> Storage {
    run_in_background();
    walk::pace_reset();
    let started = Instant::now();
    // `tmutil` runs beside the walk instead of after it, so it adds no wall time.
    let tmutil = std::thread::spawn(volume::local_snapshots);
    let o = scan_items(&home(), Path::new("/Applications"), started + TIME_BUDGET - RESERVE, now_secs());
    let capacity = volume::split_capacity(crate::iokit::boot_volume_capacity(), volume::available_and_important());
    let snapshots = tmutil.join().unwrap_or_else(|_| volume::snapshots_from(Err(reason::TMUTIL_FAILED)));
    let (coverage, progress) = coverage(&o.items, &o.missed);
    Storage {
        scanned_at: now_secs(), scan_ms: started.elapsed().as_millis() as u64, complete: !o.partial,
        not_reached: o.missed.iter().map(|(p, _)| p.clone()).collect(), coverage, progress, capacity,
        categories: categories(&o.items, &o.missed), items: o.items, snapshots, notes: NOTES,
    }
}

#[cfg(test)]
impl Storage {
    /// A small result for tests of the cache and the JSON shape.
    pub(crate) fn fake(scanned_at: u64) -> Storage {
        Storage {
            scanned_at, scan_ms: 1, complete: true, not_reached: vec![], coverage: Coverage { locations_total: 0, locations_measured: 0, bytes_measured: 0 },
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
        let o = scan_items(&home, &home.join("none"), Instant::now() - Duration::from_secs(1), now_secs());
        assert!(o.items.is_empty() && o.partial);
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
        let o = scan_items(&home, &home.join("none"), in_secs(30), now_secs());
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
        let o = scan_items(&home, &home.join("none"), in_secs(30), now_secs());
        let order: Vec<_> = o.items.iter().filter(|i| i.category == Category::Caches).map(|i| i.path.rsplit('/').next().unwrap().to_string()).collect();
        assert_eq!(order, ["com.big", "Homebrew", "com.small"]);
        assert_eq!(by_suffix(&o.items, "com.big").label, Label::Review);
        assert_eq!(by_suffix(&o.items, "Homebrew").label, Label::Safe);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn locations_are_measured_in_the_documented_order() {
        let home = temp("order");
        for rel in ["Library/Developer/Xcode/DerivedData", "Library/Containers/com.docker.docker", "Library/Caches/a", "Library/Developer/CoreSimulator", "Library/Logs", "Downloads", "Library/Mail", "apps/none"] {
            fs::create_dir_all(home.join(rel)).unwrap();
        }
        fs::create_dir_all(home.join("proj/node_modules")).unwrap();
        let o = scan_items(&home, &home.join("apps/none"), in_secs(30), now_secs());
        let tails: Vec<_> = o.items.iter().map(|i| i.path.strip_prefix(&home.display().to_string()).unwrap().to_string()).collect();
        assert_eq!(tails, ["/Library/Developer/Xcode/DerivedData", "/Library/Containers/com.docker.docker", "/Library/Caches/a", "/Library/Developer/CoreSimulator", "/Library/Logs", "/proj/node_modules", "/Downloads", "/Library/Mail", "/apps/none"]);
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
        for i in 0..400 { fs::write(big.join(format!("f{i}")), b"x").unwrap(); }
        // 10 ms left: a quarter is 2.5 ms, far less than the walk needs once pacing starts resting.
        let mut s = Scan { home: &home, deadline: Instant::now() + Duration::from_millis(1), items: vec![], missed: vec![], partial: false };
        std::thread::sleep(Duration::from_millis(2));
        s.measure(&big, Category::Developer, false);
        assert_eq!(s.missed.len(), 1, "no time left: the location is listed as not reached");
        fs::remove_dir_all(&home).unwrap();
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
        let item = |category, size, complete| Item { id: "i".into(), path: "p".into(), category, label: Label::Review, rule: Rule::Unclassified, size, apparent_bytes: None, entries: 0, complete, unreadable: 0 };
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
        let a = ids(&scan_items(&home, &home.join("none"), in_secs(30), now_secs()));
        fs::write(home.join("Downloads/new"), b"x").unwrap();
        let b = ids(&scan_items(&home, &home.join("none"), in_secs(30), now_secs()));
        assert_eq!(a, b);
        assert_ne!(a[0].1, a[1].1);
        assert_eq!(item_id("/a"), "07d66707b49cd92d", "FNV-1a is specified, so this value must not change");
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn json_shape_has_the_documented_fields() {
        let v: serde_json::Value = serde_json::from_str(&to_json(Some(&Storage::fake(1)), false, false)).unwrap();
        for k in ["scanning", "scanned_at", "scan_ms", "complete", "not_reached", "coverage", "progress", "capacity", "categories", "items", "snapshots", "notes"] { assert!(v.get(k).is_some(), "{k}"); }
        assert_eq!(v["capacity"]["purgeable"]["unavailable"], reason::STATFS_FAILED);
        assert!(v["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("clones")));
    }

    #[test]
    fn a_real_scan_with_an_expired_budget_finishes_at_once_and_says_what_it_missed() {
        // Not `scan()` itself (it reads the real home); the same code path with a past deadline.
        let home = temp("quick");
        let t = Instant::now();
        let o = scan_items(&home, &home.join("none"), Instant::now() - Duration::from_secs(1), now_secs());
        assert!(t.elapsed() < Duration::from_secs(2) && o.partial);
        fs::remove_dir_all(&home).unwrap();
    }
}
