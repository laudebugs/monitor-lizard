//! Where Monitor Lizard keeps its state, with a one-time migration from the app's earlier names.
use std::fs;
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
pub fn data_dir() -> PathBuf {
    data_dir_in(&home())
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
    }
    new
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let to = dst.join(entry.file_name());
        // Symlinks are skipped, not followed: neither baseline.json nor history.db is ever a symlink in
        // normal use, and following one could copy from or write to somewhere outside the data directory.
        if ft.is_symlink() { continue; }
        if ft.is_dir() { copy_dir(&entry.path(), &to)?; } else { fs::copy(entry.path(), &to)?; }
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
}
