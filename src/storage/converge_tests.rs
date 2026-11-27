//! Tests of the scan across runs: visit order, reuse by modification time, the round-robin refresh,
//! merging, and the per-bundle caps. Everything runs on folders under the temp directory.
use super::tests::temp;
use super::*;
use results::Entry;
use std::fs;

fn in_secs(s: u64) -> Instant { Instant::now() + Duration::from_secs(s) }

fn by_suffix<'a>(items: &'a [Item], suffix: &str) -> &'a Item { items.iter().find(|i| i.path.ends_with(suffix)).unwrap_or_else(|| panic!("no item ending {suffix}")) }

fn at_path(home: &Path, rel: &str) -> String { home.join(rel).display().to_string() }

fn entry(bytes: u64, complete: bool, measured_at: u64, mtime_hint: Option<u64>) -> Entry {
    Entry { bytes, apparent: Some(bytes), entries: 1, unreadable: 0, complete, stale: false, measured_at, mtime_hint }
}

fn order_tails(home: &Path, results: &Results, now: u64) -> Vec<(String, Rank)> {
    let (cands, _) = discover(home, &home.join("none"), in_secs(30), now, 1000);
    let prefix = format!("{}/", home.display());
    visit_order(&cands, results, now).into_iter().map(|(i, r)| (cands[i].path.display().to_string().replace(&prefix, ""), r)).collect()
}

#[test]
fn locations_are_visited_never_measured_first_then_stale_by_size_then_unchanged() {
    let home = temp("visit-order");
    for rel in ["Library/Logs", "Library/Caches/a", "Library/Caches/b", "Downloads", "Library/Mail"] { fs::create_dir_all(home.join(rel)).unwrap(); }
    let mut r = Results::default();
    r.insert(&at_path(&home, "Library/Logs"), entry(5, true, 100, trash::modified_ns(&home.join("Library/Logs"))));
    r.insert(&at_path(&home, "Library/Caches/a"), entry(100, false, 100, None));
    r.insert(&at_path(&home, "Library/Caches/b"), entry(900, false, 100, None));
    r.insert(&at_path(&home, "Library/Mail"), entry(50, true, 100, Some(1)));
    let o = order_tails(&home, &r, 150);
    let pos = |name: &str| o.iter().position(|(p, _)| p == name).unwrap_or_else(|| panic!("{name} in {o:?}"));
    let rank = |name: &str| o[pos(name)].1;
    assert_eq!((rank("Downloads"), rank("Library/Caches/b"), rank("Library/Mail"), rank("Library/Logs")), (Rank::Never, Rank::Stale, Rank::Stale, Rank::Fresh));
    assert!(pos("Downloads") < pos("Library/Caches/b"), "never measured before stale");
    assert!(pos("Library/Caches/b") < pos("Library/Caches/a") && pos("Library/Caches/a") < pos("Library/Mail"), "stale ones largest known size first");
    assert!(pos("Library/Mail") < pos("Library/Logs"), "unchanged last");
    assert!(o.iter().filter(|(_, r)| *r == Rank::Never).map(|(p, _)| pos(p)).all(|i| i < pos("Library/Caches/b")), "every never-measured location is ahead of every stale one");
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn an_unchanged_location_is_reused_and_a_deep_change_waits_for_its_turn_in_the_round_robin() {
    let home = temp("reuse");
    let logs = home.join("Library/Logs");
    fs::create_dir_all(logs.join("sub")).unwrap();
    fs::write(logs.join("sub/f"), vec![1u8; 4096]).unwrap();
    let (apps, t) = (home.join("none"), now_secs());
    let mut r = Results::default();
    let first = scan_items(&home, &apps, in_secs(30), t, &mut r);
    let before = by_suffix(&first.items, "Library/Logs");
    assert!(!before.reused && before.complete && before.measured_at == t);
    let Reading::Value(old) = before.size else { panic!("measured") };
    // Grows deep inside; the folder's own modification time does not move.
    fs::write(logs.join("sub/f"), vec![1u8; 256 * 1024]).unwrap();
    let second = scan_items(&home, &apps, in_secs(30), t + 10, &mut r);
    let reused = by_suffix(&second.items, "Library/Logs");
    assert!(reused.reused && reused.measured_at == t, "marked with when it was measured");
    assert_eq!(reused.size, Reading::Value(old), "not walked, so the growth is not seen yet");
    assert!(second.complete(), "a reused complete location still counts as complete");
    let third = scan_items(&home, &apps, in_secs(30), t + REFRESH_MIN_AGE_SECS + 10, &mut r);
    let found = by_suffix(&third.items, "Library/Logs");
    assert!(!found.reused && found.measured_at == t + REFRESH_MIN_AGE_SECS + 10);
    assert!(matches!(found.size, Reading::Value(b) if b > old), "found when its turn came");
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn each_scan_refreshes_the_two_oldest_of_the_largest_unchanged_locations() {
    let home = temp("round-robin");
    let mut r = Results::default();
    let mut names = Vec::new();
    for (i, n) in ["a", "b", "c", "d"].iter().enumerate() {
        let rel = format!("Library/Caches/com.{n}");
        fs::create_dir_all(home.join(&rel)).unwrap();
        // Same size for all four, measured at different times: a is the oldest.
        r.insert(&at_path(&home, &rel), entry(1000, true, 1000 + i as u64, trash::modified_ns(&home.join(&rel))));
        names.push(rel);
    }
    let now = 1000 + 3 * REFRESH_MIN_AGE_SECS;
    let stale: Vec<_> = order_tails(&home, &r, now).into_iter().filter(|(p, _)| names.contains(p)).filter(|(_, k)| *k == Rank::Stale).map(|(p, _)| p).collect();
    assert_eq!(stale.len(), REFRESH_PER_SCAN);
    assert!(stale.contains(&names[0]) && stale.contains(&names[1]), "the two oldest: {stale:?}");
    // Too recent for a refresh: everything is reused.
    let none = order_tails(&home, &r, 1010).into_iter().filter(|(_, k)| *k == Rank::Stale).count();
    assert_eq!(none, 0);
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_partial_walk_of_a_changed_location_keeps_the_earlier_complete_size_and_marks_it_stale() {
    let home = temp("partial-merge");
    let app = home.join("Library/Caches/com.x");
    fs::create_dir_all(&app).unwrap();
    for i in 0..50 { fs::write(app.join(format!("f{i}")), vec![1u8; 4096]).unwrap(); }
    let (apps, t) = (home.join("none"), now_secs());
    let mut r = Results::default();
    let first = scan_items_with(&home, &apps, in_secs(30), t, &mut r, 1000);
    let Reading::Value(full) = by_suffix(&first.items, "com.x").size else { panic!() };
    fs::write(app.join("new"), b"x").unwrap();
    let second = scan_items_with(&home, &apps, in_secs(30), t + 5, &mut r, 10);
    let x = by_suffix(&second.items, "com.x");
    assert_eq!(x.size, Reading::Value(full), "never lowered");
    assert!(x.stale && !x.complete && x.reused && x.measured_at == t);
    assert!(!second.complete(), "a stale value is not a complete result");
    assert!(categories(&second.items, &second.missed).iter().find(|c| c.category == Category::Caches).unwrap().lower_bound);
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_scan_out_of_time_shows_what_earlier_scans_knew_and_lists_only_what_was_never_measured() {
    let home = temp("out-of-time");
    fs::create_dir_all(home.join("Library/Logs")).unwrap();
    fs::write(home.join("Library/Logs/f"), b"x").unwrap();
    let (apps, t) = (home.join("none"), now_secs());
    let mut r = Results::default();
    scan_items(&home, &apps, in_secs(30), t, &mut r);
    fs::write(home.join("Library/Logs/g"), b"x").unwrap();
    fs::create_dir_all(home.join("Downloads/new")).unwrap();
    let o = scan_items(&home, &apps, Instant::now() - Duration::from_secs(1), t + 5, &mut r);
    let logs = by_suffix(&o.items, "Library/Logs");
    assert!(logs.reused && logs.stale && !logs.complete);
    assert!(!o.missed.iter().any(|(p, _)| p.ends_with("Library/Logs")));
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_location_deleted_since_the_last_scan_is_dropped_from_the_map() {
    let home = temp("deleted");
    fs::create_dir_all(home.join("Library/Logs")).unwrap();
    fs::create_dir_all(home.join("Downloads")).unwrap();
    let (apps, t) = (home.join("none"), now_secs());
    let mut r = Results::default();
    scan_items(&home, &apps, in_secs(30), t, &mut r);
    assert!(r.get(&at_path(&home, "Library/Logs")).is_some());
    fs::remove_dir_all(home.join("Library/Logs")).unwrap();
    let o = scan_items(&home, &apps, in_secs(30), t + 5, &mut r);
    assert!(r.get(&at_path(&home, "Library/Logs")).is_none() && !o.items.iter().any(|i| i.path.ends_with("Library/Logs")));
    assert!(r.get(&at_path(&home, "Downloads")).is_some());
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn each_app_bundle_has_its_own_entry_cap_so_the_folder_can_exceed_the_cap_in_total() {
    let home = temp("bundles");
    let apps = home.join("Applications");
    for (name, files) in [("A.app", 30), ("B.app", 30), ("C.app", 30), ("Huge.app", 100)] {
        fs::create_dir_all(apps.join(name)).unwrap();
        for i in 0..files { fs::write(apps.join(name).join(format!("f{i}")), b"x").unwrap(); }
    }
    let o = scan_items_with(&home, &apps, in_secs(30), now_secs(), &mut Results::default(), 40);
    let in_apps: Vec<_> = o.items.iter().filter(|i| i.category == Category::Applications).collect();
    assert_eq!(in_apps.len(), 4, "one item per bundle, not one for the folder");
    assert!(in_apps.iter().filter(|i| i.complete).count() == 3 && !by_suffix(&o.items, "Huge.app").complete);
    assert!(in_apps.iter().map(|i| i.entries).sum::<u64>() > 40, "the folder's total is past one cap");
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn repeated_scans_converge_to_complete_and_the_saved_map_carries_over() {
    let home = temp("converge");
    let store = home.join("data/r.json");
    fs::create_dir_all(home.join("Library/Logs")).unwrap();
    fs::create_dir_all(home.join("Downloads")).unwrap();
    let (apps, t) = (home.join("none"), now_secs());
    let mut r = Results::default();
    // An already-expired budget measures nothing; the second scan, in a new process, does.
    let cut = scan_items(&home, &apps, Instant::now() - Duration::from_secs(1), t, &mut r);
    assert!(!cut.complete());
    let o = scan_items(&home, &apps, in_secs(30), t + 1, &mut r);
    assert!(o.complete());
    r.save(&store).unwrap();
    let mut again = Results::load(&store);
    let o = scan_items(&home, &apps, in_secs(30), t + 2, &mut again);
    assert!(o.complete() && o.items.iter().all(|i| i.reused), "everything came from the saved map");
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_location_that_changed_after_the_order_was_decided_is_walked_not_reused() {
    let home = temp("changed-meanwhile");
    let logs = home.join("Library/Logs");
    fs::create_dir_all(&logs).unwrap();
    fs::write(logs.join("f"), vec![1u8; 8192]).unwrap();
    let text = logs.display().to_string();
    let mut r = Results::default();
    // The map says "complete, unchanged" but with a time the folder does not have.
    r.insert(&text, Entry { bytes: 1, apparent: Some(1), entries: 1, unreadable: 0, complete: true, stale: false, measured_at: 1, mtime_hint: Some(1) });
    let mut s = Scan { home: &home, deadline: in_secs(30), items: vec![], missed: vec![], max_entries: 1000, now: now_secs() };
    s.visit(&Candidate { path: logs.clone(), category: Category::Logs, project_idle: false }, Rank::Fresh, &mut r);
    let item = &s.items[0];
    assert!(!item.reused && matches!(item.size, Reading::Value(b) if b >= 8192));
    fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_location_deleted_while_the_scan_is_out_of_time_is_dropped_not_shown() {
    let home = temp("deleted-late");
    let logs = home.join("Library/Logs");
    fs::create_dir_all(&logs).unwrap();
    let (apps, t) = (home.join("none"), now_secs());
    let mut r = Results::default();
    scan_items(&home, &apps, in_secs(30), t, &mut r);
    fs::remove_dir_all(&logs).unwrap();
    let o = scan_items(&home, &apps, Instant::now() - Duration::from_secs(1), t + 5, &mut r);
    assert!(r.get(&logs.display().to_string()).is_none() && !o.items.iter().any(|i| i.path.ends_with("Library/Logs")));
    fs::remove_dir_all(&home).unwrap();
}
