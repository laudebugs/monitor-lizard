//! Moving scanned items to the Trash: the only way this app removes files. Every guard here runs
//! on the server, because the page that asks is untrusted. A request carries ids from the latest
//! scan, never paths, and each item is checked again against the disk as it is now.
use super::{walk::{self, Limits}, Item, Label, MAX_ENTRIES_PER_LOCATION};
use crate::sampler::Reading;
use serde::Serialize;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::{Duration, Instant};

/// The most ids one request may name.
pub const MAX_IDS: usize = 50;
/// Re-measuring an item must not hold a request for long.
const REMEASURE_BUDGET: Duration = Duration::from_secs(10);

pub mod reason {
    pub const GONE: &str = "no longer exists; rescan";
    pub const SYMLINK: &str = "is a symbolic link now, so it was not moved";
    pub const OUTSIDE: &str = "is outside the home folder and /Applications, so it was not moved";
    pub const YOURS: &str = "is labelled yours and needs the second confirmation";
    pub const UNMEASURED: &str = "was not measured completely, so a change cannot be ruled out; rescan";
    pub const CHANGED: &str = "changed since the scan; rescan";
    pub const NOT_ATTEMPTED: &str = "not attempted: an earlier move in this request failed";
}

/// Nanoseconds since the epoch of the path's own modification (a symlink is not followed).
pub fn modified_ns(p: &Path) -> Option<u64> {
    let m = std::fs::symlink_metadata(p).ok()?;
    Some((m.mtime().max(0) as u64).saturating_mul(1_000_000_000).saturating_add(m.mtime_nsec().max(0) as u64))
}

/// Ids are the 16 hex digits `item_id` makes. Anything else is refused before a lookup.
pub fn parse_ids(raw: &str) -> Result<Vec<String>, String> {
    let mut ids: Vec<String> = Vec::new();
    for id in raw.split(',').filter(|s| !s.is_empty()) {
        if id.len() != 16 || !id.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(format!("bad id '{}'", id.chars().take(20).collect::<String>())); }
        if !ids.iter().any(|i| i == id) { ids.push(id.to_string()); }
    }
    if ids.is_empty() || ids.len() > MAX_IDS { return Err(format!("expected 1 to {MAX_IDS} ids")); }
    Ok(ids)
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Entry { pub id: String, pub path: String, pub bytes: u64, pub label: Label }

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Refusal { pub id: String, pub path: String, pub reason: String }

/// What a request did. In a dry run `moved` is what would move and nothing was touched.
#[derive(Serialize, Debug, Default)]
pub struct Outcome { pub dry_run: bool, pub moved: Vec<Entry>, pub refused: Vec<Refusal>, pub bytes: u64 }

/// Why this item must not be moved right now, or its size when it may. The cheap checks of the path
/// itself come first and the re-measure last.
fn check(item: &Item, home: &Path, apps: &Path, confirm_yours: bool) -> Result<u64, String> {
    let path = Path::new(&item.path);
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(reason::GONE.into()),
        Err(e) => return Err(walk::io_reason(&e).into()),
    };
    if meta.file_type().is_symlink() { return Err(reason::SYMLINK.into()); }
    // The path must be its own canonical form: no symlinked folder above it and no `..` in it.
    let canonical = path.canonicalize().map_err(|e| walk::io_reason(&e).to_string())?;
    let inside = |root: &Path| root.canonicalize().is_ok_and(|r| canonical != r && canonical.starts_with(&r));
    if canonical != path || !(inside(home) || inside(apps)) { return Err(reason::OUTSIDE.into()); }
    if item.label == Label::Yours && !confirm_yours { return Err(reason::YOURS.into()); }
    let (Reading::Value(scanned), true) = (&item.size, item.complete) else { return Err(reason::UNMEASURED.into()) };
    if item.modified_ns.is_none() || modified_ns(path) != item.modified_ns { return Err(reason::CHANGED.into()); }
    let lim = Limits { deadline: Instant::now() + REMEASURE_BUDGET, max_entries: MAX_ENTRIES_PER_LOCATION };
    match walk::walk(path, lim) {
        None => Err(reason::GONE.into()),
        // Allocated bytes are in whole blocks and a folder's own time ignores edits inside a file, so
        // the exact length and the entry count must match as well. A same-length overwrite still passes;
        // the Trash is the backstop for that.
        Some(Ok(w)) if w.stop.is_none() && w.unreadable == 0 && w.allocated == *scanned && Some(w.apparent) == item.apparent_bytes && w.entries == item.entries => Ok(*scanned),
        Some(Ok(w)) if w.stop.is_some() || w.unreadable > 0 => Err(reason::UNMEASURED.into()),
        Some(_) => Err(reason::CHANGED.into()),
    }
}

/// Checks every item, then (unless `dry`) moves the ones that passed with `mover`, in request order.
/// A failed move stops the request: the rest are listed as not attempted, so a half-failed batch never
/// looks like it finished.
pub fn run(items: &[Item], home: &Path, apps: &Path, confirm_yours: bool, dry: bool, mover: &dyn Fn(&Path) -> Result<(), String>) -> Outcome {
    let mut out = Outcome { dry_run: dry, ..Outcome::default() };
    let mut failed = false;
    for item in items {
        let refuse = |reason: String| Refusal { id: item.id.clone(), path: item.path.clone(), reason };
        if failed { out.refused.push(refuse(reason::NOT_ATTEMPTED.into())); continue; }
        let accepted = |bytes| Entry { id: item.id.clone(), path: item.path.clone(), bytes, label: item.label };
        match check(item, home, apps, confirm_yours) {
            Err(why) => out.refused.push(refuse(why)),
            Ok(bytes) if dry => { out.bytes += bytes; out.moved.push(accepted(bytes)); }
            Ok(bytes) => match mover(Path::new(&item.path)) {
                Ok(()) => { out.bytes += bytes; out.moved.push(accepted(bytes)); }
                Err(why) => { failed = true; out.refused.push(refuse(why)); }
            },
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{item_id, Cache, Category, Rule, Storage};
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;

    fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ml-trash-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p.canonicalize().unwrap()
    }

    /// An item as a scan of `path` would have produced it.
    fn scanned(path: &Path, label: Label) -> Item {
        let text = path.display().to_string();
        let w = walk::walk(path, Limits { deadline: Instant::now() + Duration::from_secs(5), max_entries: 1000 }).unwrap().unwrap();
        Item { id: item_id(&text), path: text, category: Category::Caches, label, rule: Rule::AppCache, size: Reading::Value(w.allocated), apparent_bytes: Some(w.apparent), entries: w.entries, complete: true, unreadable: 0, modified_ns: modified_ns(path) }
    }

    /// A mover that records what it was given and removes it, in place of the Trash.
    fn recording(log: &RefCell<Vec<PathBuf>>) -> impl Fn(&Path) -> Result<(), String> + '_ {
        move |p| { log.borrow_mut().push(p.to_path_buf()); fs::remove_dir_all(p).or_else(|_| fs::remove_file(p)).map_err(|e| e.to_string()) }
    }

    fn cache_dir(home: &Path, name: &str) -> PathBuf {
        let d = home.join("Library/Caches").join(name);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("blob"), vec![1u8; 40_000]).unwrap();
        d
    }

    fn item_bytes(i: &Item) -> u64 { if let Reading::Value(b) = i.size { b } else { 0 } }

    #[test]
    fn a_safe_item_moves_and_is_reported_with_its_size() {
        let (home, apps) = (temp("moves"), temp("moves-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        let log = RefCell::new(vec![]);
        let o = run(&[item.clone()], &home, &apps, false, false, &recording(&log));
        assert_eq!((o.moved.len(), o.refused.len(), o.bytes), (1, 0, item_bytes(&item)));
        assert_eq!(*log.borrow(), vec![d.clone()]);
        assert!(!d.exists());
    }

    #[test]
    fn a_dry_run_reports_and_touches_nothing() {
        let (home, apps) = (temp("dry"), temp("dry-apps"));
        let d = cache_dir(&home, "app");
        let log = RefCell::new(vec![]);
        let o = run(&[scanned(&d, Label::Safe)], &home, &apps, false, true, &recording(&log));
        assert!(o.dry_run && o.moved.len() == 1 && o.bytes > 0);
        assert!(log.borrow().is_empty() && d.exists());
    }

    #[test]
    fn a_path_replaced_by_a_symlink_after_the_scan_is_refused() {
        let (home, apps) = (temp("symlink"), temp("symlink-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        let precious = home.join("precious");
        fs::create_dir_all(&precious).unwrap();
        fs::remove_dir_all(&d).unwrap();
        std::os::unix::fs::symlink(&precious, &d).unwrap();
        let log = RefCell::new(vec![]);
        let o = run(&[item], &home, &apps, false, false, &recording(&log));
        assert!(o.moved.is_empty() && o.refused[0].reason == reason::SYMLINK, "{o:?}");
        assert!(log.borrow().is_empty() && precious.exists());
    }

    #[test]
    fn a_symlinked_folder_above_the_item_is_refused() {
        let (home, apps, elsewhere) = (temp("above"), temp("above-apps"), temp("above-else"));
        let real = cache_dir(&elsewhere, "app");
        fs::create_dir_all(home.join("Library")).unwrap();
        std::os::unix::fs::symlink(elsewhere.join("Library/Caches"), home.join("Library/Caches")).unwrap();
        let via = home.join("Library/Caches/app");
        let item = scanned(&via, Label::Safe);
        let o = run(&[item], &home, &apps, false, false, &|_| panic!("must not move"));
        assert_eq!(o.refused[0].reason, reason::OUTSIDE);
        assert!(real.exists());
    }

    #[test]
    fn an_item_that_changed_after_the_scan_is_refused_even_within_the_same_second() {
        let (home, apps) = (temp("changed"), temp("changed-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        // Rewriting a file leaves the folder's own time alone; the size check catches it.
        fs::write(d.join("blob"), vec![2u8; 400_000]).unwrap();
        let o = run(&[item], &home, &apps, false, false, &|_| panic!("must not move"));
        assert_eq!(o.refused[0].reason, reason::CHANGED);
        assert!(d.exists());
    }

    #[test]
    fn a_few_bytes_appended_inside_the_same_block_are_noticed() {
        let (home, apps) = (temp("append"), temp("append-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        let mut f = fs::OpenOptions::new().append(true).open(d.join("blob")).unwrap();
        std::io::Write::write_all(&mut f, b"more").unwrap();
        let o = run(&[item], &home, &apps, false, false, &|_| panic!("must not move"));
        assert_eq!(o.refused[0].reason, reason::CHANGED);
    }

    #[test]
    fn an_item_with_a_new_modification_time_is_refused() {
        let (home, apps) = (temp("mtime"), temp("mtime-apps"));
        let d = cache_dir(&home, "app");
        let mut item = scanned(&d, Label::Safe);
        item.modified_ns = item.modified_ns.map(|n| n - 1);
        let o = run(&[item], &home, &apps, false, false, &|_| panic!("must not move"));
        assert_eq!(o.refused[0].reason, reason::CHANGED);
    }

    #[test]
    fn a_yours_item_needs_the_second_confirmation() {
        let (home, apps) = (temp("yours"), temp("yours-apps"));
        let d = cache_dir(&home, "mine");
        let item = scanned(&d, Label::Yours);
        let o = run(&[item.clone()], &home, &apps, false, false, &|_| panic!("must not move"));
        assert_eq!(o.refused[0].reason, reason::YOURS);
        let log = RefCell::new(vec![]);
        let o = run(&[item], &home, &apps, true, false, &recording(&log));
        assert_eq!((o.moved.len(), o.refused.len()), (1, 0));
    }

    #[test]
    fn an_item_that_was_not_fully_measured_is_refused() {
        let (home, apps) = (temp("partial"), temp("partial-apps"));
        let d = cache_dir(&home, "app");
        let mut item = scanned(&d, Label::Safe);
        item.complete = false;
        assert_eq!(run(&[item.clone()], &home, &apps, false, false, &|_| panic!("no")).refused[0].reason, reason::UNMEASURED);
        item.complete = true;
        item.size = Reading::Unavailable("permission denied");
        assert_eq!(run(&[item], &home, &apps, false, false, &|_| panic!("no")).refused[0].reason, reason::UNMEASURED);
    }

    #[test]
    fn a_listed_path_outside_home_and_applications_is_refused() {
        let (home, apps) = (temp("outside"), temp("outside-apps"));
        let other = temp("outside-other");
        let f = other.join("file");
        fs::write(&f, "x").unwrap();
        for p in [f.clone(), PathBuf::from("/etc/hosts"), home.clone(), apps.clone()] {
            let o = run(&[scanned(&p, Label::Safe)], &home, &apps, true, false, &|_| panic!("must not move {p:?}"));
            assert_eq!(o.refused[0].reason, reason::OUTSIDE, "{p:?}");
        }
        assert!(f.exists());
    }

    #[test]
    fn an_item_under_applications_may_move() {
        let (home, apps) = (temp("apps-home"), temp("apps"));
        let app = apps.join("Old.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(app.join("Contents/x"), vec![0u8; 5000]).unwrap();
        let log = RefCell::new(vec![]);
        let o = run(&[scanned(&app, Label::Review)], &home, &apps, false, false, &recording(&log));
        assert_eq!(o.moved.len(), 1, "{o:?}");
    }

    #[test]
    fn an_item_that_is_gone_is_reported() {
        let (home, apps) = (temp("gone"), temp("gone-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        fs::remove_dir_all(&d).unwrap();
        assert_eq!(run(&[item], &home, &apps, false, false, &|_| panic!("no")).refused[0].reason, reason::GONE);
    }

    #[test]
    fn a_failed_move_stops_the_batch_and_lists_the_rest_as_not_attempted() {
        let (home, apps) = (temp("half"), temp("half-apps"));
        let items: Vec<Item> = ["a", "b", "c"].iter().map(|n| scanned(&cache_dir(&home, n), Label::Safe)).collect();
        let seen = RefCell::new(0);
        let mover = |p: &Path| { *seen.borrow_mut() += 1; if *seen.borrow() == 2 { Err("Operation not permitted".to_string()) } else { fs::remove_dir_all(p).map_err(|e| e.to_string()) } };
        let o = run(&items, &home, &apps, false, false, &mover);
        assert_eq!(o.moved.len(), 1);
        assert_eq!(o.refused.iter().map(|r| r.reason.as_str()).collect::<Vec<_>>(), ["Operation not permitted", reason::NOT_ATTEMPTED]);
        assert_eq!(*seen.borrow(), 2);
        assert!(Path::new(&items[2].path).exists());
    }

    #[test]
    fn one_refusal_does_not_stop_the_others() {
        let (home, apps) = (temp("mixed"), temp("mixed-apps"));
        let a = scanned(&cache_dir(&home, "a"), Label::Yours);
        let b = scanned(&cache_dir(&home, "b"), Label::Safe);
        let log = RefCell::new(vec![]);
        let o = run(&[a, b], &home, &apps, false, false, &recording(&log));
        assert_eq!((o.moved.len(), o.refused.len()), (1, 1));
    }

    #[test]
    fn ids_must_be_sixteen_hex_digits_unique_and_few() {
        assert_eq!(parse_ids("0123456789abcdef,0123456789abcdef").unwrap().len(), 1);
        for bad in ["", ",", "xyz", "0123456789abcde", "0123456789abcdeg", "../../etc/passwd", "0123456789abcdef/0123"] { assert!(parse_ids(bad).is_err(), "{bad}"); }
        let many = (0..=MAX_IDS).map(|i| format!("{i:016x}")).collect::<Vec<_>>().join(",");
        assert!(parse_ids(&many).is_err());
    }

    #[test]
    fn a_crafted_id_cannot_reach_a_path_the_scan_did_not_list() {
        let home = temp("craft");
        let d = cache_dir(&home, "app");
        let mut s = Storage::fake(1);
        s.items.push(scanned(&d, Label::Safe));
        let cache = Cache::with_result(s);
        // The id of a path nobody scanned, built with the same hash.
        for path in ["/etc", "/etc/hosts", "/Users/someone/Documents"] {
            assert!(cache.find(&[item_id(path)]).unwrap_err().contains("not in the latest scan"), "{path}");
        }
        assert_eq!(cache.find(&[item_id(&d.display().to_string())]).unwrap().len(), 1);
        assert!(Cache::default().find(&["0123456789abcdef".into()]).is_err(), "no scan yet");
    }
}
