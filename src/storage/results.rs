//! What earlier scans learned, per location, so repeated scans converge on a full result instead of
//! starting from zero. The map is merged into, never replaced by one partial scan, and may be saved
//! under the data folder. It holds sizes and paths only: the scan stays read-only.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;

/// Bumped when the saved shape changes. A file with a larger number came from a newer build and is ignored.
const VERSION: u32 = 1;
/// A saved map larger than this is not read: a real one is a few hundred locations.
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// What is known about one location.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub bytes: u64,
    pub apparent: Option<u64>,
    pub entries: u64,
    pub unreadable: u32,
    /// True only when the walk reached the end of the location and read all of it.
    pub complete: bool,
    /// True when a later, partial measurement could not confirm an earlier complete value, so the value
    /// is kept but is old.
    pub stale: bool,
    /// Unix seconds of the measurement this entry comes from.
    pub measured_at: u64,
    /// The location's own modification time (ns) then. A hint only: a change deep inside leaves it alone.
    pub mtime_hint: Option<u64>,
}

/// One new measurement, before it is merged.
pub struct Fresh { pub bytes: u64, pub apparent: Option<u64>, pub entries: u64, pub unreadable: u32, pub complete: bool, pub mtime_hint: Option<u64> }

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Results { entries: BTreeMap<String, Entry> }

#[derive(Deserialize)]
struct File { version: u32, entries: BTreeMap<String, Entry> }

/// `File` for writing, borrowing the map so a save copies nothing.
#[derive(Serialize)]
struct FileRef<'a> { version: u32, entries: &'a BTreeMap<String, Entry> }

impl Entry {
    /// True when this complete measurement still describes the location: its own modification time is
    /// what it was then. A hint only, since a change deep inside leaves the top-level time alone.
    pub fn unchanged_at(&self, mtime: Option<u64>) -> bool { self.complete && self.mtime_hint.is_some() && self.mtime_hint == mtime }
}

impl Results {
    pub fn get(&self, path: &str) -> Option<&Entry> { self.entries.get(path) }

    #[cfg(test)]
    pub fn len(&self) -> usize { self.entries.len() }

    #[cfg(test)]
    pub fn insert(&mut self, path: &str, e: Entry) { self.entries.insert(path.to_string(), e); }

    /// Merges a measurement and returns what the map now holds. A complete measurement replaces the
    /// entry (a folder that shrank really did shrink). A partial one never lowers what is known: it
    /// keeps the larger size, stays incomplete, and marks an earlier complete value stale.
    pub fn merge(&mut self, path: &str, f: Fresh, now: u64) -> Entry {
        let new = Entry { bytes: f.bytes, apparent: f.apparent, entries: f.entries, unreadable: f.unreadable, complete: f.complete, stale: false, measured_at: now, mtime_hint: f.mtime_hint };
        let merged = match self.entries.get(path) {
            Some(old) if !f.complete && old.bytes >= f.bytes => Entry { complete: false, stale: old.complete || old.stale, ..old.clone() },
            Some(old) if !f.complete => Entry { stale: old.complete || old.stale, ..new },
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

    /// Reads a saved map. A missing, unreadable, oversized, foreign-owned, corrupt or newer-version file
    /// gives an empty map, so the scan starts from nothing rather than trusting it.
    pub fn load(path: &Path) -> Results {
        Self::try_load(path).unwrap_or_default()
    }

    fn try_load(path: &Path) -> Option<Results> {
        use std::os::unix::fs::MetadataExt;
        let mut f = crate::paths::open_nofollow(path).ok()?;
        let meta = f.metadata().ok()?;
        if !meta.is_file() || meta.uid() != crate::paths::uid() || meta.len() > MAX_FILE_BYTES { return None; }
        let mut text = String::new();
        f.read_to_string(&mut text).ok()?;
        let file: File = serde_json::from_str(&text).ok()?;
        (file.version <= VERSION).then_some(Results { entries: file.entries })
    }

    /// Writes the map owner-only (0600) in an owner-only folder (0700), through a temporary file and a
    /// rename so a crash never leaves half a file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let dir = path.parent().ok_or_else(|| std::io::Error::other("no folder to save in"))?;
        crate::paths::create_private_dir(dir)?;
        let tmp = path.with_extension("json.tmp");
        let body = serde_json::to_vec(&FileRef { version: VERSION, entries: &self.entries }).map_err(std::io::Error::other)?;
        let mut f = crate::paths::create_private_file_truncating(&tmp)?;
        f.write_all(&body)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fresh(bytes: u64, complete: bool) -> Fresh { Fresh { bytes, apparent: Some(bytes), entries: 3, unreadable: 0, complete, mtime_hint: Some(7) } }

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
}
