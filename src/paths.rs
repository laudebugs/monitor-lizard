//! Where Monitor Lizard keeps its state, with a one-time migration from the app's earlier names.
use std::fs;
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Every name this app's data folder has ever had, newest-first, excluding the current one
/// (`NEW_NAME`): a machine that skipped straight from the oldest name to today (never ran the
/// in-between version) still migrates correctly, since each is tried in turn as a possible source.
const OLDER_NAMES: &[&str] = &[".sys-monitor", ".ownwatch"];
const NEW_NAME: &str = ".monitor-lizard";

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// The data folder is `0700` and every file in it is `0600`: the history holds the user's sudo and
/// su launches, and a folder made by an older version, or copied from one, was left world-readable.
pub(crate) const DIR_MODE: u32 = 0o700;
pub(crate) const FILE_MODE: u32 = 0o600;

/// The effective user id, to compare with the owner of each entry.
pub(crate) fn uid() -> u32 {
    // SAFETY: geteuid takes no arguments, reads no memory and cannot fail.
    unsafe { libc::geteuid() }
}

/// Creates `dir` and any missing parents with mode `0700`. An existing folder is left as it is.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<()> {
    fs::DirBuilder::new().recursive(true).mode(DIR_MODE).create(dir)
}

/// Opens `path` for writing, creating it with mode `0600` from the start so the contents are never
/// readable by others, not even briefly. Contents of an existing file are kept, and so is its mode
/// (`tighten_dir` fixes old modes).
pub(crate) fn create_private_file(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new().write(true).create(true).truncate(false).mode(FILE_MODE).open(path)
}

/// Like `create_private_file`, but empties an existing file.
pub(crate) fn create_private_file_truncating(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new().write(true).create(true).truncate(true).mode(FILE_MODE).open(path)
}

/// Like `create_private_file`, but fails if `path` exists, so a name another process or a link
/// already holds is never reused.
pub(crate) fn create_new_private_file(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new().write(true).create_new(true).mode(FILE_MODE).open(path)
}

/// An error that names what was being done and to which path.
pub(crate) fn with_context(what: &str, path: &Path, e: io::Error) -> io::Error {
    io::Error::new(e.kind(), format!("could not {what} {}: {e}", path.display()))
}

/// The baseline and history folder. If it doesn't exist yet, the first of `OLDER_NAMES` found is
/// copied in once (so a machine still on the oldest name migrates directly, with no intermediate
/// folder created for a name nothing reads anymore); every older folder found is left in place.
///
/// This is the one place the folder is created (0700) and an existing folder and its files are
/// made owner-only, the first time a process asks for its location. Other modules never create it.
pub fn data_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| data_dir_in(&home())).clone()
}

/// Whether one entry may be changed, and if not, why. Symlinks, special files and the wrong kind
/// are `Skip` and never mentioned.
#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum Verdict { Tighten, Fine, Skip, Foreign, Linked }

/// Judges `meta` (from `lstat`, so a symlink shows as one). Any bit beyond `target`, including
/// setuid, setgid and sticky, counts as wider. A file with other names (`nlink > 1`) is left alone
/// because a mode change would also reach the other names, which may be outside the folder.
pub(crate) fn verdict(meta: &fs::Metadata, uid: u32, target: u32, want_dir: bool) -> Verdict {
    let ft = meta.file_type();
    if ft.is_symlink() || ft.is_fifo() || ft.is_socket() || ft.is_block_device() || ft.is_char_device() || ft.is_dir() != want_dir { return Verdict::Skip; }
    if meta.mode() & 0o7777 & !target == 0 { return Verdict::Fine; }
    if meta.uid() != uid { return Verdict::Foreign; }
    if !want_dir && meta.nlink() > 1 { return Verdict::Linked; }
    Verdict::Tighten
}

pub(crate) fn needs_tightening(meta: &fs::Metadata, uid: u32, target: u32, want_dir: bool) -> bool {
    verdict(meta, uid, target, want_dir) == Verdict::Tighten
}

/// The two operations that touch the disk, replaceable so a test can fail a chmod or swap a file
/// for a symlink between the check and the open.
pub(crate) struct Ops<'a> {
    pub chmod: &'a dyn Fn(&fs::File, u32) -> io::Result<()>,
    pub open: &'a dyn Fn(&Path) -> io::Result<fs::File>,
}

/// Opens `path` without following a final symlink, so a link swapped in after the check is refused
/// instead of followed. Non-blocking so a FIFO cannot hang the open.
pub(crate) fn open_nofollow(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)
}

fn set_mode(f: &fs::File, mode: u32) -> io::Result<()> { f.set_permissions(fs::Permissions::from_mode(mode)) }

pub(crate) const REAL: Ops<'static> = Ops { chmod: &set_mode, open: &open_nofollow };

/// What one pass over the folder did. Counts only, never file names or contents.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Tightened {
    pub dir_changed: bool,
    pub files_changed: usize,
    pub failed: usize,
    pub linked: usize,
    /// Entries (the folder counts) owned by someone else that are wider than the target.
    pub foreign: usize,
    /// The data folder is a symlink, so nothing in it was looked at.
    pub dir_is_link: bool,
    pub first_error: Option<String>,
}

impl Tightened {
    /// The lines to print: one for a change, one for anything left undone; nothing when every mode
    /// was already tight.
    pub fn lines(&self, dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        if self.dir_changed || self.files_changed > 0 {
            let files = if self.files_changed > 0 { format!(" and {} file(s) in it", self.files_changed) } else { String::new() };
            out.push(format!("monitor-lizard: made {}{files} owner-only; other users can no longer read the history", dir.display()));
        }
        let mut left = Vec::new();
        if self.dir_is_link { left.push("the folder is a symlink, so its contents were not looked at".to_string()); }
        if self.failed > 0 { left.push(format!("{} item(s) could not be changed ({})", self.failed, self.first_error.as_deref().unwrap_or("unknown error"))); }
        if self.linked > 0 { left.push(format!("{} linked file(s) left alone", self.linked)); }
        if self.foreign > 0 { left.push(format!("{} item(s) belong to another user and stay readable by others; fix with: sudo chown -R $USER '{}'", self.foreign, dir.display())); }
        if !left.is_empty() { out.push(format!("monitor-lizard: in {}: {}; continuing", dir.display(), left.join("; "))); }
        out
    }
}

/// Sets `path` to `target` when it needs it. The mode is set on the opened handle, never by name,
/// and the handle is checked to be the same file that was judged.
fn tighten_one(path: &Path, want_dir: bool, uid: u32, target: u32, ops: &Ops, t: &mut Tightened) -> bool {
    let r = (|| -> io::Result<bool> {
        let meta = fs::symlink_metadata(path)?;
        match verdict(&meta, uid, target, want_dir) {
            Verdict::Fine | Verdict::Skip => return Ok(false),
            Verdict::Foreign => { t.foreign += 1; return Ok(false); }
            Verdict::Linked => { t.linked += 1; return Ok(false); }
            Verdict::Tighten => {}
        }
        let f = (ops.open)(path)?;
        let now = f.metadata()?;
        if now.dev() != meta.dev() || now.ino() != meta.ino() || !needs_tightening(&now, uid, target, want_dir) { return Ok(false); }
        (ops.chmod)(&f, target)?;
        Ok(true)
    })();
    match r {
        Ok(changed) => changed,
        Err(e) => { t.failed += 1; t.first_error.get_or_insert_with(|| e.to_string()); false }
    }
}

/// Makes an existing data folder `0700` and each regular file directly inside it `0600`. Symlinks,
/// special files, files with other names and anything owned by someone else are not changed. A
/// failure is counted and the pass carries on, since the program works without it.
pub(crate) fn tighten_dir(dir: &Path, uid: u32, ops: &Ops) -> Tightened {
    let mut t = Tightened::default();
    // A symlinked folder is left alone: listing it would reach files outside the data folder.
    match fs::symlink_metadata(dir) {
        Ok(m) if m.file_type().is_symlink() => { t.dir_is_link = true; return t; }
        Ok(m) if m.is_dir() => {}
        _ => return t,
    }
    t.dir_changed = tighten_one(dir, true, uid, DIR_MODE, ops, &mut t);
    match fs::read_dir(dir) {
        Ok(entries) => for e in entries.flatten() { if tighten_one(&e.path(), false, uid, FILE_MODE, ops, &mut t) { t.files_changed += 1; } },
        Err(e) => { t.failed += 1; t.first_error.get_or_insert_with(|| e.to_string()); }
    }
    t
}

fn data_dir_in(base: &Path) -> PathBuf {
    let new = base.join(NEW_NAME);
    if !new.exists() {
        if let Some(old) = OLDER_NAMES.iter().map(|n| base.join(n)).find(|p| p.exists()) {
            if let Err(e) = copy_dir(&old, &new) {
                eprintln!("monitor-lizard: could not migrate {} to {}: {e}; starting with empty state", old.display(), new.display());
                let _ = fs::remove_dir_all(&new); // don't leave a half-copied folder behind
            }
        }
        // Created here, owner-only, so no later code ever makes it with the umask's mode.
        if let Err(e) = create_private_dir(&new) { eprintln!("monitor-lizard: could not create {}: {e}", new.display()); }
    }
    for line in tighten_dir(&new, uid(), &REAL).lines(&new) { eprintln!("{line}"); }
    new
}

fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    create_private_dir(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let to = dst.join(entry.file_name());
        // Symlinks are skipped, not followed: neither baseline.json nor history.db is ever a symlink in
        // normal use, and following one could copy from or write to somewhere outside the data directory.
        if ft.is_symlink() { continue; }
        if ft.is_dir() { copy_dir(&entry.path(), &to)?; } else if ft.is_file() {
            // Not fs::copy, which carries the old mode over: the copy must be owner-only whatever the source was.
            io::copy(&mut fs::File::open(entry.path())?, &mut create_new_private_file(&to)?)?;
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

    fn mode(p: &Path) -> u32 { fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777 }
    fn put(p: &Path, text: &str, m: u32) { fs::write(p, text).unwrap(); fs::set_permissions(p, fs::Permissions::from_mode(m)).unwrap(); }
    fn old_folder(base: &Path, name: &str) -> PathBuf {
        let d = base.join(name);
        fs::create_dir_all(&d).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o755)).unwrap();
        for f in ["history.db", "history.db-wal", "history.db-shm", "baseline.json"] { put(&d.join(f), "data", 0o644); }
        d
    }
    fn meta(p: &Path) -> fs::Metadata { fs::symlink_metadata(p).unwrap() }

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
    fn a_fresh_machine_gets_a_0700_folder_from_the_one_function_everything_calls() {
        let base = tmp("fresh-mode");
        let d = data_dir_in(&base);
        assert_eq!(mode(&d), 0o700);
        // What each module does next, on the folder this function made.
        crate::settings::save_at_for_test(&d.join("settings.json"));
        crate::history::History::at(&d.join("history.db")).unwrap();
        assert_eq!(mode(&d), 0o700);
        assert_eq!(mode(&d.join("settings.json")), 0o600);
        assert_eq!(mode(&d.join("history.db")), 0o600);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn the_report_counts_files_not_the_folder_and_a_second_run_is_silent() {
        let base = tmp("idempotent");
        let d = old_folder(&base, NEW_NAME);
        let first = tighten_dir(&d, uid(), &REAL);
        assert!(first.dir_changed);
        assert_eq!(first.files_changed, 4);
        let lines = first.lines(&d);
        assert_eq!(lines.len(), 1, "one line for the whole pass");
        assert!(lines[0].contains("and 4 file(s) in it"), "{}", lines[0]);
        let second = tighten_dir(&d, uid(), &REAL);
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
        let chmod = |f: &fs::File, m: u32| { rewrote.set(rewrote.get() + 1); set_mode(f, m) };
        let t = tighten_dir(&d, uid(), &Ops { chmod: &chmod, open: &open_nofollow });
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
        tighten_dir(&d, uid(), &REAL);
        assert_eq!(mode(&outside), 0o644, "the target outside the folder is untouched");
        assert!(meta(&d.join("link")).file_type().is_symlink());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_symlinked_data_folder_is_not_listed_or_changed_and_the_message_says_so() {
        let base = tmp("linked-dir");
        let real = old_folder(&base, "real");
        std::os::unix::fs::symlink(&real, base.join(NEW_NAME)).unwrap();
        let d = data_dir_in(&base);
        assert_eq!(mode(&real), 0o755);
        assert_eq!(mode(&real.join("baseline.json")), 0o644);
        let t = tighten_dir(&d, uid(), &REAL);
        assert!(t.dir_is_link);
        assert!(t.lines(&d)[0].contains("is a symlink"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_file_swapped_for_a_symlink_between_the_check_and_the_open_is_not_followed() {
        let base = tmp("swap");
        let d = old_folder(&base, NEW_NAME);
        let outside = base.join("outside.txt");
        put(&outside, "secret", 0o644);
        let swap_then_open = |p: &Path| {
            if p.ends_with("baseline.json") {
                fs::remove_file(p).unwrap();
                std::os::unix::fs::symlink(&outside, p).unwrap();
            }
            open_nofollow(p)
        };
        let t = tighten_dir(&d, uid(), &Ops { chmod: &set_mode, open: &swap_then_open });
        assert_eq!(mode(&outside), 0o644, "the file outside the folder was not changed");
        assert!(t.failed <= 1);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn opening_a_symlink_without_following_it_fails() {
        let base = tmp("nofollow");
        put(&base.join("t"), "x", 0o644);
        std::os::unix::fs::symlink(base.join("t"), base.join("l")).unwrap();
        assert!(open_nofollow(&base.join("l")).is_err());
        assert!(open_nofollow(&base.join("t")).is_ok());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_fifo_and_a_unix_socket_are_skipped_without_hanging() {
        let base = tmp("special");
        let d = base.join("d");
        fs::create_dir_all(&d).unwrap();
        let fifo = std::ffi::CString::new(d.join("fifo").to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);
        let _sock = std::os::unix::net::UnixListener::bind(d.join("s")).unwrap();
        fs::set_permissions(d.join("s"), fs::Permissions::from_mode(0o666)).unwrap();
        let t = tighten_dir(&d, uid(), &REAL);
        assert_eq!(t.files_changed, 0);
        assert_eq!(mode(&d.join("fifo")), 0o644);
        assert_eq!(mode(&d.join("s")), 0o666);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_setuid_file_loses_the_bit_even_when_the_other_bits_are_tight() {
        let base = tmp("setuid");
        let d = base.join("d");
        fs::create_dir_all(&d).unwrap();
        fs::set_permissions(&d, fs::Permissions::from_mode(0o700)).unwrap();
        put(&d.join("a"), "x", 0o4755);
        put(&d.join("b"), "x", 0o4600);
        tighten_dir(&d, uid(), &REAL);
        assert_eq!(mode(&d.join("a")), 0o600);
        assert_eq!(mode(&d.join("b")), 0o600);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_hard_link_to_a_file_elsewhere_is_left_alone_and_counted() {
        let base = tmp("hardlink");
        let d = old_folder(&base, NEW_NAME);
        let outside = base.join("outside.txt");
        put(&outside, "mine", 0o644);
        fs::hard_link(&outside, d.join("linked")).unwrap();
        let t = tighten_dir(&d, uid(), &REAL);
        assert_eq!(mode(&outside), 0o644, "the other name of the file is untouched");
        assert_eq!(t.linked, 1);
        assert_eq!(t.files_changed, 4);
        assert!(t.lines(&d).iter().any(|l| l.contains("1 linked file(s) left alone")));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn the_predicate_skips_other_owners_symlinks_links_and_tight_modes() {
        let base = tmp("predicate");
        let f = base.join("f");
        put(&f, "x", 0o644);
        assert_eq!(verdict(&meta(&f), uid(), FILE_MODE, false), Verdict::Tighten);
        assert_eq!(verdict(&meta(&f), uid().wrapping_add(1), FILE_MODE, false), Verdict::Foreign);
        put(&f, "x", 0o600);
        assert_eq!(verdict(&meta(&f), uid(), FILE_MODE, false), Verdict::Fine);
        put(&f, "x", 0o640);
        assert!(needs_tightening(&meta(&f), uid(), FILE_MODE, false), "any group or other bit counts");
        assert_eq!(verdict(&meta(&f), uid(), FILE_MODE, true), Verdict::Skip, "a file where a folder is wanted");
        std::os::unix::fs::symlink(&f, base.join("l")).unwrap();
        assert_eq!(verdict(&meta(&base.join("l")), uid(), FILE_MODE, false), Verdict::Skip);
        fs::hard_link(&f, base.join("h")).unwrap();
        assert_eq!(verdict(&meta(&f), uid(), FILE_MODE, false), Verdict::Linked);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn entries_owned_by_someone_else_are_skipped_and_reported_with_the_folder_and_a_fix() {
        let base = tmp("foreign");
        let d = old_folder(&base, NEW_NAME);
        let t = tighten_dir(&d, uid().wrapping_add(1), &REAL);
        assert_eq!((t.dir_changed, t.files_changed, t.foreign), (false, 0, 5));
        assert_eq!(mode(&d), 0o755);
        let lines = t.lines(&d);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains(&d.display().to_string()) && lines[0].contains("sudo chown -R $USER"), "{}", lines[0]);
        assert!(!lines[0].contains("baseline.json"), "no file names");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_chmod_failure_is_counted_once_and_the_pass_continues() {
        let base = tmp("chmod-fail");
        let d = old_folder(&base, NEW_NAME);
        let fail = |_: &fs::File, _: u32| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let t = tighten_dir(&d, uid(), &Ops { chmod: &fail, open: &open_nofollow });
        assert_eq!((t.files_changed, t.failed), (0, 5), "every entry was tried despite the failures");
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
    fn private_helpers_create_0700_and_0600_and_keep_existing_contents() {
        let base = tmp("private");
        let d = base.join("a").join("b");
        create_private_dir(&d).unwrap();
        assert_eq!(mode(&d), 0o700);
        let f = base.join("f");
        create_private_file(&f).unwrap();
        assert_eq!(mode(&f), 0o600);
        fs::write(&f, "keep").unwrap();
        create_private_file(&f).unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "keep");
        assert!(create_new_private_file(&f).is_err());
        create_private_file_truncating(&f).unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "");
        let _ = fs::remove_dir_all(&base);
    }
}
