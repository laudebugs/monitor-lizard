//! What earlier scans learned, per location, so repeated scans converge on a full result instead of
//! starting from zero. The map is merged into, never replaced by one partial scan, and may be saved
//! under the data folder. It holds sizes and paths only: the scan stays read-only.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Bumped when the saved shape changes. A file with a larger number came from a newer build and is
/// neither read nor overwritten.
const VERSION: u32 = 1;
/// A saved map larger than this is not read: a real one is a few hundred locations.
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
/// A map with more entries or a longer path than this is refused whole, as a damaged or hostile file.
const MAX_ENTRIES: usize = 20_000;
const MAX_PATH_BYTES: usize = 4096;
/// A lower bound from a cut-off walk is replaced by a newer, smaller one once it is this old (seconds),
/// so a folder that shrank does not keep its old size forever.
const PARTIAL_BOUND_MAX_AGE_SECS: u64 = 7 * 24 * 3600;

/// What is known about one location.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub bytes: u64,
    pub apparent: Option<u64>,
    pub entries: u64,
    /// Subfolders the walk could not read. Their size is missing, so with more than 0 the size is
    /// "at least", but it is the size of what could be read now, not a stale larger number.
    pub unreadable: u32,
    /// True only when the walk reached the end of the location and read all of it.
    pub complete: bool,
    /// True when a later, cut-off measurement could not confirm an earlier complete value, so the
    /// value is kept but is old.
    pub stale: bool,
    /// Unix seconds of the measurement this entry comes from.
    pub measured_at: u64,
    /// The location's own modification time (ns) then. A hint only: a change deep inside leaves it alone.
    pub mtime_hint: Option<u64>,
}

/// One new measurement, before it is merged.
pub struct Fresh {
    pub bytes: u64,
    pub apparent: Option<u64>,
    pub entries: u64,
    pub unreadable: u32,
    pub complete: bool,
    /// The walk reached the end rather than being stopped by time or the entry cap. With `complete`
    /// false that means only unreadable subfolders are missing, and the size is exact for the rest.
    pub exhausted: bool,
    pub mtime_hint: Option<u64>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Results { entries: BTreeMap<String, Entry> }

#[derive(Deserialize)]
struct File { version: u32, entries: BTreeMap<String, Entry> }

/// `File` for writing, borrowing the map so a save copies nothing.
#[derive(Serialize)]
struct FileRef<'a> { version: u32, entries: &'a BTreeMap<String, Entry> }

#[derive(Deserialize)]
struct VersionOnly { version: u32 }

impl Entry {
    /// True when this complete measurement still describes the location: its own modification time is
    /// what it was then. A hint only, since a change deep inside leaves the top-level time alone.
    pub fn unchanged_at(&self, mtime: Option<u64>) -> bool { self.complete && self.mtime_hint.is_some() && self.mtime_hint == mtime }
}

/// A saved map is read only when it is a regular file of ours and not oversized.
fn acceptable(meta: &std::fs::Metadata, uid: u32) -> bool { meta.is_file() && meta.uid() == uid && meta.len() <= MAX_FILE_BYTES }

/// A path as a map key, or None when it is not valid UTF-8 (a lossy key would name another path).
pub fn key_of(path: &Path) -> Option<&str> { path.to_str() }

impl Results {
    pub fn get(&self, path: &str) -> Option<&Entry> { self.entries.get(path) }

    #[cfg(test)]
    pub fn len(&self) -> usize { self.entries.len() }

    #[cfg(test)]
    pub fn insert(&mut self, path: &str, e: Entry) { self.entries.insert(path.to_string(), e); }

    /// Merges a measurement and returns what the map now holds.
    /// - A walk that reached the end replaces the entry, even when unreadable subfolders keep it from
    ///   being complete: it is exact for what can be read, and a folder that shrank really did shrink.
    /// - A cut-off walk (time or entry cap) never lowers what is known: it keeps the larger size, stays
    ///   incomplete, and marks an earlier complete value stale. A bound older than a week is replaced.
    pub fn merge(&mut self, path: &str, f: Fresh, now: u64) -> Entry {
        let new = Entry { bytes: f.bytes, apparent: f.apparent, entries: f.entries, unreadable: f.unreadable, complete: f.complete, stale: false, measured_at: now, mtime_hint: f.mtime_hint };
        let cut_off = !f.complete && !f.exhausted;
        let merged = match self.entries.get(path) {
            Some(old) if cut_off && old.measured_at.saturating_add(PARTIAL_BOUND_MAX_AGE_SECS) >= now => {
                if old.bytes >= f.bytes { Entry { complete: false, stale: old.complete || old.stale, ..old.clone() } } else { Entry { stale: old.complete || old.stale, ..new } }
            }
            _ => new,
        };
        self.entries.insert(path.to_string(), merged.clone());
        merged
    }

    /// Records that a location could not be measured this time (denied, say), so what is known stays
    /// but no longer counts as complete. Returns the entry, or None when nothing was known.
    pub fn mark_stale(&mut self, path: &str) -> Option<Entry> {
        let e = self.entries.get_mut(path)?;
        e.stale = e.stale || e.complete;
        e.complete = false;
        Some(e.clone())
    }

    pub fn forget(&mut self, path: &str) { self.entries.remove(path); }

    /// Drops entries this scan did not see whose location no longer exists.
    pub fn prune_missing(&mut self, seen: &HashSet<String>) {
        self.entries.retain(|p, _| seen.contains(p) || std::fs::symlink_metadata(p).is_ok());
    }

    /// Reads a saved map. A missing, unreadable, oversized, foreign-owned, corrupt, newer-version or
    /// implausible file gives an empty map, so the scan starts from nothing rather than trusting it.
    pub fn load(path: &Path) -> Results { Self::try_load(path).unwrap_or_default() }

    fn try_load(path: &Path) -> Option<Results> { Self::try_load_as(path, crate::paths::uid()) }

    /// `try_load` for a given owner, so the owner check can be tested without a second user.
    fn try_load_as(path: &Path, uid: u32) -> Option<Results> {
        let f = crate::paths::open_nofollow(path).ok()?;
        if !acceptable(&f.metadata().ok()?, uid) { return None; }
        let mut text = String::new();
        f.take(MAX_FILE_BYTES + 1).read_to_string(&mut text).ok()?;
        let file: File = serde_json::from_str(&text).ok()?;
        let plausible = file.entries.len() <= MAX_ENTRIES && file.entries.keys().all(|k| k.len() <= MAX_PATH_BYTES);
        // A time in the future would win every comparison against a real measurement.
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let mut entries = file.entries;
        for e in entries.values_mut() { e.measured_at = e.measured_at.min(now); }
        (file.version <= VERSION && plausible).then_some(Results { entries })
    }

    /// The version a file on disk says it has, when it can be read at all.
    fn disk_version(path: &Path) -> Option<u32> {
        let f = crate::paths::open_nofollow(path).ok()?;
        if !acceptable(&f.metadata().ok()?, crate::paths::uid()) { return None; }
        let mut text = String::new();
        f.take(MAX_FILE_BYTES + 1).read_to_string(&mut text).ok()?;
        serde_json::from_str::<VersionOnly>(&text).ok().map(|v| v.version)
    }

    /// Writes the map owner-only (0600) in an owner-only folder (0700), without losing what another
    /// process (the command line, the server) saved since this one loaded: the file on disk is read
    /// again and merged in first, the newer measurement of a path winning. A file from a newer build is
    /// left alone and reported as an error. Two processes saving at the same moment can still lose one
    /// update: the later rename wins, and the next scan measures the location again.
    pub fn save(&mut self, path: &Path) -> std::io::Result<()> { self.save_with(path, &|f| f.sync_all()) }

    /// `save`, with the flush injected so a test can see what is flushed and in which order.
    fn save_with(&mut self, path: &Path, sync: &dyn Fn(&std::fs::File) -> std::io::Result<()>) -> std::io::Result<()> {
        let dir = path.parent().ok_or_else(|| std::io::Error::other("no folder to save in"))?;
        crate::paths::create_private_dir(dir)?;
        // Only a real folder of ours is tightened or written into; a link could point anywhere.
        let meta = std::fs::symlink_metadata(dir)?;
        if meta.file_type().is_symlink() || !meta.is_dir() || meta.uid() != crate::paths::uid() { return Err(std::io::Error::other("the folder for the saved results is a link or not yours, so nothing was saved")); }
        if meta.permissions().mode() & 0o077 != 0 { std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?; }
        if Self::disk_version(path).is_some_and(|v| v > VERSION) { return Err(std::io::Error::other("the saved results come from a newer build, so they were left as they are")); }
        if let Some(disk) = Self::try_load(path) {
            for (p, e) in disk.entries {
                let newer = self.entries.get(&p).is_none_or(|mine| e.measured_at > mine.measured_at);
                // A path only the disk knows is kept if it still exists; one this process dropped is not revived.
                if newer && std::fs::symlink_metadata(&p).is_ok() { self.entries.insert(p, e); }
            }
        }
        // The union can pass the cap the next load enforces, which would then drop the whole file.
        if self.entries.len() > MAX_ENTRIES {
            let mut oldest: Vec<(u64, String)> = self.entries.iter().map(|(p, e)| (e.measured_at, p.clone())).collect();
            oldest.sort();
            for (_, p) in oldest.into_iter().take(self.entries.len() - MAX_ENTRIES) { self.entries.remove(&p); }
        }
        let tmp = dir.join(format!(".storage-results.{}.tmp", std::process::id()));
        let written = self.write_new(&tmp, sync).and_then(|()| std::fs::rename(&tmp, path)).and_then(|()| sync(&std::fs::File::open(dir)?));
        if written.is_err() { let _ = std::fs::remove_file(&tmp); }
        written
    }

    /// A new file at `tmp`, never one that was already there: a leftover (or a link someone planted) is
    /// removed as itself, and creation fails rather than follow anything.
    fn write_new(&self, tmp: &Path, sync: &dyn Fn(&std::fs::File) -> std::io::Result<()>) -> std::io::Result<()> {
        match std::fs::remove_file(tmp) { Ok(()) => {}, Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}, Err(e) => return Err(e) }
        let body = serde_json::to_vec(&FileRef { version: VERSION, entries: &self.entries }).map_err(std::io::Error::other)?;
        let mut f = crate::paths::create_new_private_file(tmp)?;
        f.write_all(&body)?;
        sync(&f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fresh(bytes: u64, complete: bool) -> Fresh { Fresh { bytes, apparent: Some(bytes), entries: 3, unreadable: 0, complete, exhausted: complete, mtime_hint: Some(7) } }

    fn temp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("ml-results-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn a_partial_scan_never_lowers_a_complete_earlier_result() {
        let mut r = Results::default();
        r.merge("/a", fresh(1000, true), 10);
        let kept = r.merge("/a", fresh(40, false), 20);
        assert_eq!((kept.bytes, kept.complete, kept.stale, kept.measured_at), (1000, false, true, 10), "the old value is kept, stale, and not claimed complete");
        let again = r.merge("/a", fresh(60, false), 30);
        assert_eq!((again.bytes, again.stale), (1000, true), "stale stays until a complete measurement");
        assert_eq!(r.get("/a").unwrap().bytes, 1000);
    }

    #[test]
    fn a_partial_scan_that_found_more_raises_the_lower_bound_and_a_complete_one_replaces() {
        let mut r = Results::default();
        r.merge("/a", fresh(100, false), 10);
        assert_eq!(r.merge("/a", fresh(300, false), 20).bytes, 300);
        assert_eq!(r.merge("/a", fresh(200, false), 30).bytes, 300, "partials only ever raise the bound");
        let done = r.merge("/a", fresh(250, true), 40);
        assert_eq!((done.bytes, done.complete, done.stale), (250, true, false), "a complete measurement is the truth, even when smaller");
    }

    #[test]
    fn a_location_that_cannot_be_read_keeps_what_is_known_but_not_as_complete() {
        let mut r = Results::default();
        assert!(r.mark_stale("/none").is_none());
        r.merge("/a", fresh(500, true), 10);
        let e = r.mark_stale("/a").unwrap();
        assert_eq!((e.bytes, e.complete, e.stale), (500, false, true));
    }

    #[test]
    fn deleted_locations_are_dropped_and_present_ones_kept() {
        let dir = temp("prune");
        std::fs::create_dir_all(dir.join("here")).unwrap();
        let (here, gone, seen) = (dir.join("here").display().to_string(), dir.join("gone").display().to_string(), dir.join("seen").display().to_string());
        let mut r = Results::default();
        for p in [&here, &gone, &seen] { r.merge(p, fresh(1, true), 1); }
        r.prune_missing(&HashSet::from([seen.clone()]));
        assert!(r.get(&here).is_some() && r.get(&seen).is_some() && r.get(&gone).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_saved_map_is_owner_only_in_an_owner_only_folder_and_reads_back() {
        let dir = temp("save");
        let file = dir.join("data/storage-results.json");
        let mut r = Results::default();
        r.merge("/a", fresh(9, true), 5);
        r.save(&file).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(file.parent().unwrap()), 0o700);
        assert!(!file.with_extension("json.tmp").exists(), "no temporary file is left behind");
        assert_eq!(Results::load(&file).get("/a"), r.get("/a"));
        r.save(&file).unwrap();
        assert_eq!(mode(&file), 0o600, "a second save keeps the mode");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_corrupt_or_newer_or_missing_file_is_ignored() {
        let dir = temp("bad");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("r.json");
        assert_eq!(Results::load(&f).len(), 0, "missing");
        for body in ["", "not json", "{\"version\":1}", "{\"version\":1,\"entries\":{\"/a\":{\"bytes\":\"x\"}}}"] {
            std::fs::write(&f, body).unwrap();
            assert_eq!(Results::load(&f).len(), 0, "{body:?}");
        }
        let mut r = Results::default();
        r.merge("/a", fresh(9, true), 5);
        r.save(&f).unwrap();
        assert_eq!(Results::load(&f).len(), 1, "a good file loads");
        let newer = std::fs::read_to_string(&f).unwrap().replacen("\"version\":1", &format!("\"version\":{}", VERSION + 1), 1);
        std::fs::write(&f, newer).unwrap();
        assert_eq!(Results::load(&f).len(), 0, "a newer build's file is ignored");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_symlink_in_place_of_the_file_is_not_followed() {
        let dir = temp("link");
        std::fs::create_dir_all(&dir).unwrap();
        let real = dir.join("real.json");
        let mut r = Results::default();
        r.merge("/a", fresh(9, true), 5);
        r.save(&real).unwrap();
        let link = dir.join("link.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(Results::load(&link).len(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_walk_that_reached_the_end_replaces_the_size_even_with_unreadable_children() {
        let mut r = Results::default();
        r.merge("/a", fresh(1000, true), 10);
        let shrunk = r.merge("/a", Fresh { unreadable: 1, exhausted: true, ..fresh(300, false) }, 20);
        assert_eq!((shrunk.bytes, shrunk.complete, shrunk.stale, shrunk.unreadable), (300, false, false, 1), "exact for what can be read, marked incomplete, not a stale larger number");
        let again = r.merge("/a", Fresh { unreadable: 1, exhausted: true, ..fresh(200, false) }, 30);
        assert_eq!(again.bytes, 200, "and it keeps falling with the folder");
    }

    #[test]
    fn an_old_lower_bound_from_a_cut_off_walk_ages_out() {
        let mut r = Results::default();
        r.merge("/a", fresh(1000, false), 10);
        assert_eq!(r.merge("/a", fresh(40, false), 20).bytes, 1000, "a recent bound stays");
        let week = PARTIAL_BOUND_MAX_AGE_SECS;
        assert_eq!(r.merge("/a", fresh(40, false), 10 + week + 1).bytes, 40, "a week-old bound is replaced");
    }

    #[test]
    fn an_oversized_file_is_ignored_even_when_its_content_is_good() {
        let dir = temp("oversize");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("r.json");
        let mut r = Results::default();
        r.merge("/a", fresh(9, true), 5);
        r.save(&f).unwrap();
        assert_eq!(Results::load(&f).len(), 1);
        let mut body = std::fs::read(&f).unwrap();
        body.resize(MAX_FILE_BYTES as usize + 1, b' ');
        std::fs::write(&f, body).unwrap();
        assert_eq!(Results::load(&f).len(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_a_regular_file_of_ours_is_acceptable() {
        let dir = temp("owner");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("r.json");
        std::fs::write(&f, "{}").unwrap();
        let meta = std::fs::metadata(&f).unwrap();
        assert!(acceptable(&meta, crate::paths::uid()));
        assert!(!acceptable(&meta, crate::paths::uid().wrapping_add(1)), "someone else's file");
        assert!(!acceptable(&std::fs::metadata(&dir).unwrap(), crate::paths::uid()), "a folder");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn too_many_entries_or_a_very_long_path_make_the_whole_file_untrusted() {
        let dir = temp("implausible");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("r.json");
        let e = |bytes| format!("{{\"bytes\":{bytes},\"apparent\":null,\"entries\":0,\"unreadable\":0,\"complete\":true,\"stale\":false,\"measured_at\":1,\"mtime_hint\":null}}");
        let many = (0..=MAX_ENTRIES).map(|i| format!("\"/p{i}\":{}", e(1))).collect::<Vec<_>>().join(",");
        std::fs::write(&f, format!("{{\"version\":1,\"entries\":{{{many}}}}}")).unwrap();
        assert_eq!(Results::load(&f).len(), 0, "past the entry cap");
        std::fs::write(&f, format!("{{\"version\":1,\"entries\":{{\"/{}\":{}}}}}", "x".repeat(MAX_PATH_BYTES), e(1))).unwrap();
        assert_eq!(Results::load(&f).len(), 0, "a path past the length cap");
        std::fs::write(&f, format!("{{\"version\":1,\"entries\":{{\"/ok\":{}}}}}", e(1))).unwrap();
        assert_eq!(Results::load(&f).len(), 1, "the same shape within the caps loads");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saving_tightens_a_folder_that_was_wider_and_leaves_no_temporary_file() {
        let dir = temp("wide");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut r = Results::default();
        r.merge("/a", fresh(9, true), 5);
        r.save(&dir.join("r.json")).unwrap();
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "only the results file is left");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_leftover_or_planted_temporary_file_is_replaced_and_never_written_through() {
        let dir = temp("leftover");
        std::fs::create_dir_all(&dir).unwrap();
        let victim = dir.join("victim");
        std::fs::write(&victim, "keep me").unwrap();
        let tmp = dir.join(format!(".storage-results.{}.tmp", std::process::id()));
        std::os::unix::fs::symlink(&victim, &tmp).unwrap();
        let mut r = Results::default();
        r.merge("/a", fresh(9, true), 5);
        r.save(&dir.join("r.json")).unwrap();
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep me", "the link's target is untouched");
        std::fs::write(&tmp, "stale junk").unwrap();
        r.save(&dir.join("r.json")).unwrap();
        assert_eq!(Results::load(&dir.join("r.json")).len(), 1);
        assert!(!tmp.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failed_save_removes_its_temporary_file() {
        let dir = temp("failed");
        std::fs::create_dir_all(&dir).unwrap();
        let mut r = Results::default();
        r.merge("/a", fresh(9, true), 5);
        let err = r.save_with(&dir.join("r.json"), &|_| Err(std::io::Error::other("disk full")));
        assert!(err.is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "nothing left behind");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_save_flushes_the_file_and_then_the_folder() {
        let dir = temp("sync");
        std::fs::create_dir_all(&dir).unwrap();
        let mut r = Results::default();
        let calls = std::cell::RefCell::new(Vec::new());
        r.save_with(&dir.join("r.json"), &|f| { calls.borrow_mut().push(f.metadata().unwrap().is_dir()); Ok(()) }).unwrap();
        assert_eq!(*calls.borrow(), [false, true], "file first, then the folder that holds the rename");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_from_a_newer_build_is_not_overwritten() {
        let dir = temp("newer-keep");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("r.json");
        let newer = format!("{{\"version\":{},\"entries\":{{}},\"future\":true}}", VERSION + 1);
        std::fs::write(&f, &newer).unwrap();
        let mut r = Results::default();
        r.merge("/a", fresh(9, true), 5);
        assert!(r.save(&f).is_err());
        assert_eq!(std::fs::read_to_string(&f).unwrap(), newer);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn two_processes_saving_do_not_lose_each_others_learning() {
        let dir = temp("two-writers");
        std::fs::create_dir_all(dir.join("x")).unwrap();
        std::fs::create_dir_all(dir.join("y")).unwrap();
        let (x, y) = (dir.join("x").display().to_string(), dir.join("y").display().to_string());
        let f = dir.join("r.json");
        let (mut server, mut cli) = (Results::default(), Results::default());
        server.merge(&x, fresh(10, true), 100);
        cli.merge(&y, fresh(20, true), 100);
        server.save(&f).unwrap();
        cli.save(&f).unwrap();
        let back = Results::load(&f);
        assert!(back.get(&x).is_some() && back.get(&y).is_some(), "the later writer kept what the earlier one learned");
        // A newer measurement on disk beats an older one in memory.
        let mut late = Results::default();
        late.merge(&x, fresh(99, true), 500);
        late.save(&f).unwrap();
        server.save(&f).unwrap();
        assert_eq!(Results::load(&f).get(&x).unwrap().bytes, 99);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_path_that_is_not_utf8_has_no_key() {
        use std::os::unix::ffi::OsStrExt;
        assert_eq!(key_of(Path::new("/a/b")), Some("/a/b"));
        assert_eq!(key_of(Path::new(std::ffi::OsStr::from_bytes(b"/a/\xff\xfe"))), None);
    }

    #[test]
    fn the_owner_check_applies_on_the_load_path() {
        let dir = temp("load-owner");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("r.json");
        let mut r = Results::default();
        r.merge("/a", fresh(9, true), 5);
        r.save(&f).unwrap();
        assert_eq!(Results::try_load_as(&f, crate::paths::uid()).map(|r| r.len()), Some(1));
        assert!(Results::try_load_as(&f, crate::paths::uid().wrapping_add(1)).is_none(), "a file that is not ours is not read");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_future_time_on_disk_does_not_beat_a_real_measurement() {
        let dir = temp("future");
        std::fs::create_dir_all(dir.join("x")).unwrap();
        let x = dir.join("x").display().to_string();
        let f = dir.join("r.json");
        let mut disk = Results::default();
        disk.merge(&x, fresh(1, true), u64::MAX - 1);
        disk.entries.get_mut(&x).unwrap().measured_at = u64::MAX - 1;
        disk.save(&f).unwrap();
        let mut mine = Results::default();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        mine.merge(&x, fresh(77, true), now + 10);
        mine.save(&f).unwrap();
        assert_eq!(Results::load(&f).get(&x).unwrap().bytes, 77);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_union_past_the_cap_drops_the_oldest_so_the_file_still_loads() {
        let dir = temp("union-cap");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("r.json");
        let mut r = Results::default();
        for i in 0..MAX_ENTRIES + 50 { r.insert(&format!("/p{i}"), Entry { bytes: 1, apparent: None, entries: 0, unreadable: 0, complete: true, stale: false, measured_at: i as u64, mtime_hint: None }); }
        r.save(&f).unwrap();
        let back = Results::load(&f);
        assert_eq!(back.len(), MAX_ENTRIES, "the file is still trusted");
        assert!(back.get("/p0").is_none() && back.get(&format!("/p{}", MAX_ENTRIES + 49)).is_some(), "the oldest went first");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_folder_that_is_a_link_is_neither_tightened_nor_written_into() {
        let dir = temp("link-dir");
        let real = dir.join("real");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755)).unwrap();
        let link = dir.join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let mut r = Results::default();
        r.merge("/a", fresh(9, true), 5);
        assert!(r.save(&link.join("r.json")).is_err());
        assert_eq!(std::fs::metadata(&real).unwrap().permissions().mode() & 0o777, 0o755, "the target's mode is untouched");
        assert_eq!(std::fs::read_dir(&real).unwrap().count(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
