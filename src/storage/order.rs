//! Which locations a scan measures and in what order: the fixed list, the entries of `/Applications`,
//! and the ranking against what earlier scans learned.
use super::results::{key_of, Results};
use super::{slice, trash, walk, Category};
use std::path::{Path, PathBuf};
use std::time::Instant;
use walk::Limits;

/// One location to measure, before the order is known.
pub(super) struct Candidate { pub path: PathBuf, pub category: Category, pub project_idle: bool }

/// Where a location stands against what earlier scans learned. The derived order is the visit order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Rank {
    /// Never measured: walked first, in the fixed order.
    Never,
    /// Unchanged but due for its turn in the refresh of the long tail: walked next, so a scan that runs
    /// out of time still reaches it.
    Due,
    /// Incomplete before, changed since, or due in the refresh of the largest: walked next, largest first.
    Stale,
    /// Complete and unchanged at the top level: reused without a walk.
    Fresh,
}


/// Every location this scan may measure, in the fixed order of usefulness per second spent. It is
/// also the visit order on a first run, and it matters because the budget usually runs out:
/// Xcode's DerivedData and the Docker folder are large and safe to act on; app caches come next,
/// biggest first by a cheap one-level size; simulators and logs follow; the `node_modules` search is
/// slow and comes after them; the user's own folders and the apps in `/Applications` cannot be
/// `safe` and go last. Each app bundle is its own location with its own entry cap.
pub(super) fn discover(home: &Path, applications: &Path, deadline: Instant, now: u64, max_entries: u64) -> (Vec<Candidate>, Vec<(String, Category)>) {
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
    // A name that is not valid UTF-8 cannot be a map key or an item path without changing it into another path.
    let (out, bad): (Vec<Candidate>, Vec<Candidate>) = out.into_iter().partition(|c| key_of(&c.path).is_some());
    for c in bad { missed.push((format!("{} (the name is not valid UTF-8)", c.path.display()), c.category)); }
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

/// How many of the largest unchanged locations form the first refresh pool, and how many of them one
/// scan walks again.
const REFRESH_POOL: usize = 8;
pub(super) const REFRESH_PER_SCAN: usize = 2;
/// Every other unchanged location is walked again too: one in this many, oldest first, each scan.
/// So a location whose top-level time never moves is still re-walked within about this many scans.
pub(super) const TAIL_SHARE: usize = 20;
/// An unchanged location is not walked again for a refresh until it is this old (seconds), so scans
/// run back to back reuse everything.
pub(super) const REFRESH_MIN_AGE_SECS: u64 = 600;

/// The order to visit `cands` in, with each one's rank. Never-measured locations come first, in the
/// fixed order; then stale ones, largest known size first; then unchanged ones, which cost nothing.
/// A change deep inside a folder whose own time did not move is found by the refresh in turn: of the
/// largest unchanged locations the two measured longest ago are walked again each scan, and of all the
/// others the oldest one in `TAIL_SHARE`, so every location is re-walked within about that many scans
/// that are at least ten minutes apart. With `full`, nothing is reused: the owner asked for a fresh scan.
pub(super) fn visit_order(cands: &[Candidate], results: &Results, now: u64, full: bool) -> Vec<(usize, Rank)> {
    // (index, rank, last known size, measured_at)
    let mut ranked: Vec<(usize, Rank, u64, u64)> = cands.iter().enumerate().map(|(i, c)| match key_of(&c.path).and_then(|k| results.get(k)) {
        None => (i, Rank::Never, 0, 0),
        Some(e) => {
            let unchanged = !full && e.unchanged_at(trash::modified_ns(&c.path));
            (i, if unchanged { Rank::Fresh } else { Rank::Stale }, e.bytes, e.measured_at)
        }
    }).collect();
    let mut fresh: Vec<usize> = (0..ranked.len()).filter(|&k| ranked[k].1 == Rank::Fresh).collect();
    fresh.sort_by(|&a, &b| ranked[b].2.cmp(&ranked[a].2).then(ranked[a].0.cmp(&ranked[b].0)));
    let tail = fresh.split_off(fresh.len().min(REFRESH_POOL));
    let by_age = |v: &mut Vec<usize>| v.sort_by_key(|&k| (ranked[k].3, ranked[k].0));
    let (mut pool, mut tail) = (fresh, tail);
    by_age(&mut pool);
    by_age(&mut tail);
    let ages: Vec<u64> = ranked.iter().map(|r| r.3).collect();
    let old_enough = |k: &usize| now.saturating_sub(ages[*k]) >= REFRESH_MIN_AGE_SECS;
    let tail_count = tail.len().div_ceil(TAIL_SHARE);
    for k in pool.into_iter().filter(old_enough).take(REFRESH_PER_SCAN).collect::<Vec<_>>() { ranked[k].1 = Rank::Stale; }
    for k in tail.into_iter().filter(old_enough).take(tail_count).collect::<Vec<_>>() { ranked[k].1 = Rank::Due; }
    ranked.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| if a.1 == Rank::Never { a.0.cmp(&b.0) } else { b.2.cmp(&a.2).then(a.0.cmp(&b.0)) }));
    ranked.into_iter().map(|(i, r, _, _)| (i, r)).collect()
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
