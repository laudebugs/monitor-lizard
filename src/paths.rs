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
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() { copy_dir(&entry.path(), &to)?; } else { fs::copy(entry.path(), &to)?; }
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
