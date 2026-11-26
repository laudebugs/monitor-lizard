//! Where sys-monitor keeps its state, with a one-time migration from the old `ownwatch` name.
use std::fs;
use std::path::{Path, PathBuf};

const OLD_NAME: &str = ".ownwatch";
const NEW_NAME: &str = ".sys-monitor";

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// The baseline and history folder. If it doesn't exist yet but the old `.ownwatch` folder does,
/// its contents are copied once; the old folder is left in place either way.
pub fn data_dir() -> PathBuf {
    data_dir_in(&home())
}

fn data_dir_in(base: &Path) -> PathBuf {
    let new = base.join(NEW_NAME);
    let old = base.join(OLD_NAME);
    if !new.exists() && old.exists() {
        if let Err(e) = copy_dir(&old, &new) {
            eprintln!("sys-monitor: could not migrate {} to {}: {e}; starting with empty state", old.display(), new.display());
            let _ = fs::remove_dir_all(&new); // don't leave a half-copied folder behind
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
        let d = std::env::temp_dir().join(format!("sysmon-paths-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn fresh_machine_creates_only_the_new_directory() {
        let base = tmp("fresh");
        let dir = data_dir_in(&base);
        assert_eq!(dir, base.join(NEW_NAME));
        assert!(!base.join(OLD_NAME).exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn migrates_old_data_and_leaves_the_old_folder_in_place() {
        let base = tmp("migrate");
        fs::create_dir_all(base.join(OLD_NAME)).unwrap();
        fs::write(base.join(OLD_NAME).join("baseline.json"), "{\"approved\":[\"x\"]}").unwrap();
        let dir = data_dir_in(&base);
        assert_eq!(fs::read_to_string(dir.join("baseline.json")).unwrap(), "{\"approved\":[\"x\"]}");
        assert!(base.join(OLD_NAME).join("baseline.json").exists(), "the old folder is never touched");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn migration_failure_leaves_the_old_folder_untouched_and_logs_instead_of_panicking() {
        let base = tmp("fail");
        fs::create_dir_all(base.join(OLD_NAME)).unwrap();
        fs::write(base.join(OLD_NAME).join("baseline.json"), "{}").unwrap();
        // Make the base read-only so creating the new folder inside it fails with a permission error.
        let mut perms = fs::metadata(&base).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&base, perms.clone()).unwrap();
        let dir = data_dir_in(&base); // must not panic
        perms.set_readonly(false);
        fs::set_permissions(&base, perms).unwrap(); // restore so the test can clean up
        assert!(!dir.exists() || fs::read_dir(&dir).unwrap().next().is_none(), "no half-migrated folder is left behind");
        assert_eq!(fs::read_to_string(base.join(OLD_NAME).join("baseline.json")).unwrap(), "{}", "the old folder is untouched by a failed migration");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn symlinks_in_the_old_folder_are_skipped_not_followed() {
        let base = tmp("symlink");
        fs::create_dir_all(base.join(OLD_NAME)).unwrap();
        fs::write(base.join(OLD_NAME).join("baseline.json"), "real").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(base.join(OLD_NAME).join("baseline.json"), base.join(OLD_NAME).join("link.json")).unwrap();
        let dir = data_dir_in(&base);
        assert_eq!(fs::read_to_string(dir.join("baseline.json")).unwrap(), "real");
        assert!(!dir.join("link.json").exists(), "the symlink is skipped, not copied or followed");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn both_present_uses_the_new_one_unchanged() {
        let base = tmp("both");
        fs::create_dir_all(base.join(OLD_NAME)).unwrap();
        fs::write(base.join(OLD_NAME).join("baseline.json"), "old").unwrap();
        fs::create_dir_all(base.join(NEW_NAME)).unwrap();
        fs::write(base.join(NEW_NAME).join("baseline.json"), "new").unwrap();
        let dir = data_dir_in(&base);
        assert_eq!(fs::read_to_string(dir.join("baseline.json")).unwrap(), "new");
        let _ = fs::remove_dir_all(&base);
    }
}
