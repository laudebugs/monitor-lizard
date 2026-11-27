use super::History;
use rusqlite::Connection;
use std::path::PathBuf;

    pub(super) fn version_of(path: &std::path::Path) -> usize {
        super::migrations::user_version(&Connection::open(path).unwrap()).unwrap()
    }

    pub(super) fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ow-test-migrate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub(super) fn tmp_history(name: &str) -> (History, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("ow-test-metrics-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        (History::at(&dir.join("h.db")).unwrap(), dir)
    }
