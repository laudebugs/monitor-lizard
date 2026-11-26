//! Where Monitor Lizard keeps its state, with a one-time migration from the app's earlier names.
use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Every name this app's data folder has ever had, newest-first, excluding the current one
/// (`NEW_NAME`): a machine that skipped straight from the oldest name to today (never ran the
/// in-between version) still migrates correctly, since each is tried in turn as a possible source.
const OLDER_NAMES: &[&str] = &[".sys-monitor", ".ownwatch"];
const NEW_NAME: &str = ".monitor-lizard";

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// The baseline and history folder. If it doesn't exist yet, the first of `OLDER_NAMES` found is
/// copied in once (so a machine still on the oldest name migrates directly, with no intermediate
/// folder created for a name nothing reads anymore); every older folder found is left in place.
///
/// Every entry point reads the folder through here, so this is also where an existing folder and
/// its files are made owner-only, once per process.
pub fn data_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| data_dir_in(&home())).clone()
}

/// The data folder is `0700` and every file in it is `0600`: the history holds the user's sudo and
/// su launches, and a folder made by an older version, or copied from one, was left world-readable.
const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

/// Creates `path` for writing with mode `0600` from the start, so the contents are never readable by
/// others, not even briefly. An existing file keeps its mode here; `tighten_dir` handles those.
pub fn open_private(path: &Path, truncate: bool) -> std::io::Result<fs::File> {
    fs::OpenOptions::new().write(true).create(true).truncate(truncate).mode(FILE_MODE).open(path)
}

/// What one pass over the folder did. Counts only, never names or contents.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Tightened { pub changed: usize, pub failed: usize, pub first_error: Option<String> }

impl Tightened {
    /// The lines to print: nothing when every mode was already tight.
    pub fn lines(&self, dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        if self.changed > 0 { out.push(format!("monitor-lizard: made {} (and {} item(s) in it) owner-only; other users can no longer read the history", dir.display(), self.changed)); }
        if self.failed > 0 { out.push(format!("monitor-lizard: could not make {} item(s) in {} owner-only ({}); continuing", self.failed, dir.display(), self.first_error.as_deref().unwrap_or("unknown error"))); }
        out
    }
}

/// True when `meta` describes something this pass may change: not a symlink, owned by `uid`, and
/// with permission bits beyond `target`. Narrower modes are left alone.
pub(crate) fn needs_tightening(meta: &fs::Metadata, uid: u32, target: u32) -> bool {
    !meta.file_type().is_symlink() && meta.uid() == uid && (meta.mode() & 0o777) & !target != 0
}

/// Opens `path` without following a final symlink, so a link swapped in after the check is refused
/// instead of followed. Non-blocking so a FIFO cannot hang the open.
fn open_nofollow(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)
}

/// Sets `path` to `target` when it needs it. Returns whether it changed the mode. The mode is set
/// on the opened handle and re-checked there, never by name.
fn tighten_one(path: &Path, want_dir: bool, uid: u32, target: u32, chmod: &dyn Fn(&fs::File, u32) -> std::io::Result<()>) -> std::io::Result<bool> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() != want_dir || !(meta.is_dir() || meta.is_file()) || !needs_tightening(&meta, uid, target) { return Ok(false); }
    let f = open_nofollow(path)?;
    let now = f.metadata()?;
    if now.dev() != meta.dev() || now.ino() != meta.ino() || !needs_tightening(&now, uid, target) { return Ok(false); }
    chmod(&f, target)?;
    Ok(true)
}

/// Makes an existing data folder `0700` and each regular file directly inside it `0600`. Anything
/// that is a symlink, owned by someone else, or already as tight is skipped. A failure is counted
/// and the pass carries on, since the program works without it.
pub(crate) fn tighten_dir(dir: &Path, uid: u32, chmod: &dyn Fn(&fs::File, u32) -> std::io::Result<()>) -> Tightened {
    let mut t = Tightened::default();
    let note = |t: &mut Tightened, r: std::io::Result<bool>| match r {
        Ok(true) => t.changed += 1,
        Ok(false) => {}
        Err(e) => { t.failed += 1; t.first_error.get_or_insert_with(|| e.to_string()); }
    };
    // The folder first: if it was unreadable to us nothing below could be listed anyway.
    // A symlinked folder is left alone: listing it would reach files outside the data folder.
    if !fs::symlink_metadata(dir).map(|m| m.is_dir()).unwrap_or(false) { return t; }
    note(&mut t, tighten_one(dir, true, uid, DIR_MODE, chmod));
    match fs::read_dir(dir) {
        Ok(entries) => for e in entries.flatten() { note(&mut t, tighten_one(&e.path(), false, uid, FILE_MODE, chmod)); },
        Err(e) => note(&mut t, Err(e)),
    }
    t
}

fn set_mode(f: &fs::File, mode: u32) -> std::io::Result<()> { f.set_permissions(fs::Permissions::from_mode(mode)) }

fn data_dir_in(base: &Path) -> PathBuf {
    let new = base.join(NEW_NAME);
    if !new.exists() {
        if let Some(old) = OLDER_NAMES.iter().map(|n| base.join(n)).find(|p| p.exists()) {
            if let Err(e) = copy_dir(&old, &new) {
                eprintln!("monitor-lizard: could not migrate {} to {}: {e}; starting with empty state", old.display(), new.display());
                let _ = fs::remove_dir_all(&new); // don't leave a half-copied folder behind
            }
        }
    }
    for line in tighten_dir(&new, unsafe { libc::geteuid() }, &set_mode).lines(&new) { eprintln!("{line}"); }
    new
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new().recursive(true).mode(DIR_MODE).create(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let to = dst.join(entry.file_name());
        // Symlinks are skipped, not followed: neither baseline.json nor history.db is ever a symlink in
        // normal use, and following one could copy from or write to somewhere outside the data directory.
        if ft.is_symlink() { continue; }
        if ft.is_dir() { copy_dir(&entry.path(), &to)?; } else {
            // Not fs::copy, which carries the old mode over: the copy must be owner-only whatever the source was.
            let mut from = fs::File::open(entry.path())?;
            std::io::copy(&mut from, &mut fs::OpenOptions::new().write(true).create_new(true).mode(FILE_MODE).open(&to)?)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("monlizard-paths-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn fresh_machine_creates_only_the_new_directory() {
        let base = tmp("fresh");
        let dir = data_dir_in(&base);
        assert_eq!(dir, base.join(NEW_NAME));
        for old in OLDER_NAMES { assert!(!base.join(old).exists()); }
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn migrates_from_the_most_recent_older_name_and_leaves_it_in_place() {
        let base = tmp("migrate");
        fs::create_dir_all(base.join(".sys-monitor")).unwrap();
        fs::write(base.join(".sys-monitor").join("baseline.json"), "{\"approved\":[\"x\"]}").unwrap();
        let dir = data_dir_in(&base);
        assert_eq!(fs::read_to_string(dir.join("baseline.json")).unwrap(), "{\"approved\":[\"x\"]}");
        assert!(base.join(".sys-monitor").join("baseline.json").exists(), "the old folder is never touched");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn migrates_straight_from_the_oldest_name_when_the_in_between_one_was_never_used() {
        // A machine that skipped the Sys Monitor era entirely (still on the original ownwatch name).
        let base = tmp("oldest-only");
        fs::create_dir_all(base.join(".ownwatch")).unwrap();
        fs::write(base.join(".ownwatch").join("baseline.json"), "{\"approved\":[\"y\"]}").unwrap();
        let dir = data_dir_in(&base);
        assert_eq!(fs::read_to_string(dir.join("baseline.json")).unwrap(), "{\"approved\":[\"y\"]}");
        assert!(!base.join(".sys-monitor").exists(), "no intermediate folder is created for a name nothing reads anymore");
        assert!(base.join(".ownwatch").join("baseline.json").exists(), "the oldest folder is never touched");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn both_older_names_present_migrates_from_the_newer_one_and_touches_neither() {
        // A machine that has every era's folder on disk at once: .sys-monitor is the one actually in
        // use (OLDER_NAMES tries it first), so .ownwatch's data must never be the source here.
        let base = tmp("both-older");
        fs::create_dir_all(base.join(".sys-monitor")).unwrap();
        fs::write(base.join(".sys-monitor").join("baseline.json"), "{\"approved\":[\"newer\"]}").unwrap();
        fs::create_dir_all(base.join(".ownwatch")).unwrap();
        fs::write(base.join(".ownwatch").join("baseline.json"), "{\"approved\":[\"older\"]}").unwrap();
        let dir = data_dir_in(&base);
        assert_eq!(fs::read_to_string(dir.join("baseline.json")).unwrap(), "{\"approved\":[\"newer\"]}");
        assert_eq!(fs::read_to_string(base.join(".sys-monitor").join("baseline.json")).unwrap(), "{\"approved\":[\"newer\"]}", "the newer older-folder is never touched");
        assert_eq!(fs::read_to_string(base.join(".ownwatch").join("baseline.json")).unwrap(), "{\"approved\":[\"older\"]}", "the oldest folder is never touched, and was never read from either");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn migration_failure_leaves_the_old_folder_untouched_and_logs_instead_of_panicking() {
        let base = tmp("fail");
        fs::create_dir_all(base.join(".sys-monitor")).unwrap();
        fs::write(base.join(".sys-monitor").join("baseline.json"), "{}").unwrap();
        // Make the base read-only so creating the new folder inside it fails with a permission error.
        let mut perms = fs::metadata(&base).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&base, perms.clone()).unwrap();
        let dir = data_dir_in(&base); // must not panic
        perms.set_readonly(false);
        fs::set_permissions(&base, perms).unwrap(); // restore so the test can clean up
        assert!(!dir.exists() || fs::read_dir(&dir).unwrap().next().is_none(), "no half-migrated folder is left behind");
        assert_eq!(fs::read_to_string(base.join(".sys-monitor").join("baseline.json")).unwrap(), "{}", "the old folder is untouched by a failed migration");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn symlinks_in_the_old_folder_are_skipped_not_followed() {
        let base = tmp("symlink");
        fs::create_dir_all(base.join(".sys-monitor")).unwrap();
        fs::write(base.join(".sys-monitor").join("baseline.json"), "real").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(base.join(".sys-monitor").join("baseline.json"), base.join(".sys-monitor").join("link.json")).unwrap();
        let dir = data_dir_in(&base);
        assert_eq!(fs::read_to_string(dir.join("baseline.json")).unwrap(), "real");
        assert!(!dir.join("link.json").exists(), "the symlink is skipped, not copied or followed");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn newest_folder_already_present_uses_it_unchanged_and_ignores_every_older_name() {
        let base = tmp("both");
        fs::create_dir_all(base.join(".sys-monitor")).unwrap();
        fs::write(base.join(".sys-monitor").join("baseline.json"), "old").unwrap();
        fs::create_dir_all(base.join(NEW_NAME)).unwrap();
        fs::write(base.join(NEW_NAME).join("baseline.json"), "new").unwrap();
        let dir = data_dir_in(&base);
        assert_eq!(fs::read_to_string(dir.join("baseline.json")).unwrap(), "new");
        let _ = fs::remove_dir_all(&base);
    }

    fn mode(p: &Path) -> u32 { fs::symlink_metadata(p).unwrap().permissions().mode() & 0o777 }
    fn put(p: &Path, text: &str, m: u32) { fs::write(p, text).unwrap(); fs::set_permissions(p, fs::Permissions::from_mode(m)).unwrap(); }
    fn uid() -> u32 { unsafe { libc::geteuid() } }
    fn old_folder(base: &Path, name: &str) -> PathBuf {
        let d = base.join(name);
        fs::create_dir_all(&d).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o755)).unwrap();
        for f in ["history.db", "history.db-wal", "history.db-shm", "baseline.json"] { put(&d.join(f), "data", 0o644); }
        d
    }

    #[test]
    fn an_existing_wide_folder_and_its_files_become_owner_only() {
        let base = tmp("tighten");
        let d = old_folder(&base, NEW_NAME);
        assert_eq!(data_dir_in(&base), d);
        assert_eq!(mode(&d), 0o700);
        for f in ["history.db", "history.db-wal", "history.db-shm", "baseline.json"] { assert_eq!(mode(&d.join(f)), 0o600, "{f}"); }
        assert_eq!(fs::read_to_string(d.join("baseline.json")).unwrap(), "data", "contents are never changed");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_second_run_changes_nothing_and_says_nothing() {
        let base = tmp("idempotent");
        let d = old_folder(&base, NEW_NAME);
        let first = tighten_dir(&d, uid(), &set_mode);
        assert_eq!(first.changed, 5);
        assert_eq!(first.lines(&d).len(), 1, "one line for the whole pass");
        let second = tighten_dir(&d, uid(), &set_mode);
        assert_eq!(second, Tightened::default());
        assert!(second.lines(&d).is_empty());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn already_tight_or_narrower_modes_are_not_rewritten() {
        let base = tmp("tight");
        let d = base.join("d");
        fs::create_dir_all(&d).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o700)).unwrap();
        put(&d.join("a"), "x", 0o600);
        put(&d.join("b"), "x", 0o400);
        let rewrote = std::cell::Cell::new(0);
        let t = tighten_dir(&d, uid(), &|f, m| { rewrote.set(rewrote.get() + 1); set_mode(f, m) });
        assert_eq!(rewrote.get(), 0);
        assert!(t.lines(&d).is_empty());
        assert_eq!(mode(&d.join("b")), 0o400, "a narrower mode is not widened");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn symlinks_are_neither_followed_nor_changed() {
        let base = tmp("links");
        let d = old_folder(&base, NEW_NAME);
        let outside = base.join("outside.txt");
        put(&outside, "secret", 0o644);
        std::os::unix::fs::symlink(&outside, d.join("link")).unwrap();
        tighten_dir(&d, uid(), &set_mode);
        assert_eq!(mode(&outside), 0o644, "the target outside the folder is untouched");
        assert!(fs::symlink_metadata(d.join("link")).unwrap().file_type().is_symlink());
        // A symlinked folder is not followed either.
        let real = old_folder(&base, "real");
        std::os::unix::fs::symlink(&real, base.join("linked-dir")).unwrap();
        assert_eq!(tighten_dir(&base.join("linked-dir"), uid(), &set_mode).changed, 0);
        assert_eq!(mode(&real), 0o755);
        assert_eq!(mode(&real.join("baseline.json")), 0o644);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn the_predicate_skips_other_owners_symlinks_and_tight_modes() {
        let base = tmp("predicate");
        let f = base.join("f");
        put(&f, "x", 0o644);
        let meta = fs::symlink_metadata(&f).unwrap();
        assert!(needs_tightening(&meta, uid(), FILE_MODE));
        assert!(!needs_tightening(&meta, uid().wrapping_add(1), FILE_MODE), "owned by someone else");
        put(&f, "x", 0o600);
        assert!(!needs_tightening(&fs::symlink_metadata(&f).unwrap(), uid(), FILE_MODE));
        put(&f, "x", 0o640);
        assert!(needs_tightening(&fs::symlink_metadata(&f).unwrap(), uid(), FILE_MODE), "any group or other bit counts");
        std::os::unix::fs::symlink(&f, base.join("l")).unwrap();
        assert!(!needs_tightening(&fs::symlink_metadata(base.join("l")).unwrap(), uid(), FILE_MODE));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_file_owned_by_someone_else_is_skipped_in_a_pass() {
        let base = tmp("foreign");
        let d = old_folder(&base, NEW_NAME);
        assert_eq!(tighten_dir(&d, uid().wrapping_add(1), &set_mode), Tightened::default());
        assert_eq!(mode(&d), 0o755);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_chmod_failure_is_counted_once_and_the_pass_continues() {
        let base = tmp("chmod-fail");
        let d = old_folder(&base, NEW_NAME);
        let t = tighten_dir(&d, uid(), &|_, _| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)));
        assert_eq!((t.changed, t.failed), (0, 5), "every entry was tried despite the failures");
        assert_eq!(t.lines(&d).len(), 1);
        assert!(!t.lines(&d)[0].contains("data"), "no file contents in the line");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_read_only_parent_does_not_stop_the_pass() {
        let base = tmp("ro-parent");
        let d = old_folder(&base, NEW_NAME);
        fs::set_permissions(&base, fs::Permissions::from_mode(0o500)).unwrap();
        let got = data_dir_in(&base); // must not panic
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(got, d);
        assert_eq!(mode(&d), 0o700);
        assert_eq!(mode(&d.join("history.db")), 0o600);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn the_migration_copy_is_owner_only_whatever_the_old_modes_were() {
        let base = tmp("migrate-modes");
        let old = old_folder(&base, ".sys-monitor");
        fs::create_dir_all(old.join("sub")).unwrap();
        put(&old.join("sub").join("n"), "n", 0o644);
        let dir = data_dir_in(&base);
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&dir.join("sub")), 0o700);
        assert_eq!(mode(&dir.join("sub").join("n")), 0o600);
        for f in ["history.db", "history.db-wal", "history.db-shm", "baseline.json"] { assert_eq!(mode(&dir.join(f)), 0o600, "{f}"); }
        assert_eq!(mode(&old), 0o755, "the old folder is left as it was");
        assert_eq!(mode(&old.join("baseline.json")), 0o644);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_private_file_is_created_0600_and_an_existing_one_keeps_its_contents() {
        let base = tmp("private");
        let f = base.join("f");
        open_private(&f, true).unwrap();
        assert_eq!(mode(&f), 0o600);
        fs::write(&f, "keep").unwrap();
        open_private(&f, false).unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "keep");
        let _ = fs::remove_dir_all(&base);
    }
}
