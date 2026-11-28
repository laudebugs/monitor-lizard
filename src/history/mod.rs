//! The history database (`history.db`): `History` owns the connection and the open, lock and migration
//! plumbing here; each table's code lives in its own submodule and is re-exported so callers keep the
//! `history::` paths.
mod actions;
mod baseline_changes;
mod capacity;
mod disk_writes;
mod metrics;
mod migrations;
mod persistence;
mod seen;
mod sudo;
mod trash;
mod verdicts;
#[cfg(test)]
mod test_util;

pub use actions::ActionRow;
pub use baseline_changes::BaselineChangeRow;
pub use capacity::{capacity_tick, db_path, local_day, local_utc_offset, read_capacity_series, start_capacity_writer, CapacityWrite};
pub use disk_writes::{disk_writes_hook, erase_disk_writes_now, read_disk_writes, EraseFlag, Stored, writes_reply, WriteRow, DISK_WRITES_KEEP_SECS};
pub use metrics::{start_metrics_writer, HistoryPoint};
pub use persistence::{persistence_baseline, persistence_changes, Change, ChangeKind, Observed, PersistRow, MAX_CHANGES};
pub use seen::{SeenRow, FLAG_WINDOW_SECS, LEARNING_SECS};
pub use trash::{TrashItemRow, TrashMoveRow, INTERRUPTED_REASON, TRASH_KEEP};
pub use verdicts::{VerdictRow, VERDICTS_KEEP_SECS};

use capacity::CAPACITY_TABLE;
use disk_writes::DISK_WRITES_TABLE;
use migrations::{backup_path, file_error, migrate, open_lock, remove_stale_backups, MIGRATIONS};
use rusqlite::Connection;
use std::path::PathBuf;
use trash::{TRASH_ITEMS_TABLE, TRASH_TABLE};
use verdicts::VERDICTS_TABLE;

/// What the dashboard and the command line say when the history cannot be opened.
pub fn unavailable(e: &rusqlite::Error) -> String { format!("history unavailable: {e} (database: {})", path().display()) }

/// Opens the history, or says why not (once per process, on stderr) and returns None. For callers
/// that can carry on without history; a database from a newer build would otherwise look empty.
pub fn open_or_report() -> Option<History> {
    static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    History::open().map_err(|e| {
        if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) { eprintln!("monitor-lizard: {}", unavailable(&e)); }
    }).ok()
}

pub struct History(Connection);

fn path() -> PathBuf { crate::paths::data_dir().join("history.db") }

impl History {
    pub fn open() -> rusqlite::Result<Self> { Self::at(&path()) }
    pub fn at(p: &std::path::Path) -> rusqlite::Result<Self> {
        // The data folder already exists (paths::data_dir makes it); this covers a caller-chosen path.
        if let Some(d) = p.parent() { let _ = crate::paths::create_private_dir(d); }
        // One opener at a time reads the version, backs up and migrates, across processes and across
        // threads (each open() of the lock file is its own lock owner, so threads exclude each other
        // too). Without it a second opener could read version 0 after the first had migrated and
        // back up the already-migrated file.
        let _lock = open_lock(p)?;
        remove_stale_backups(&backup_path(p));
        // Created here at 0600 so SQLite opens an existing file and the -wal and -shm files it
        // makes copy this mode; letting SQLite create it would apply the umask (usually 0644).
        crate::paths::create_private_file(p).map_err(|e| file_error("create the database file", p, e))?;
        let mut c = Connection::open(p)?;
        c.busy_timeout(std::time::Duration::from_secs(10))?;
        // Migrate before switching journal mode so a file from a newer version is refused untouched.
        migrate(&mut c, MIGRATIONS, Some(&backup_path(p)))?;
        c.execute_batch(CAPACITY_TABLE)?;
        c.execute_batch(TRASH_TABLE)?;
        c.execute_batch(TRASH_ITEMS_TABLE)?;
        c.execute_batch(DISK_WRITES_TABLE)?;
        c.execute_batch(VERDICTS_TABLE)?;
        // Best effort: a busy or read-only database must still open for the callers that only read it.
        let _ = verdicts::prune(&c, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
        // WAL so a process killed mid-batch leaves the file intact (the write-ahead log is simply
        // replayed or rolled back on next open) rather than the default rollback journal's larger
        // window for a half-written page; every table shares one connection and one journal mode.
        c.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::test_util::tmp_history;

    #[test]
    fn opening_history_turns_on_wal_mode() {
        // The actual mechanism the "process killed mid-batch" edge case relies on: a crash mid-write
        // leaves the file intact because WAL (not the default rollback journal) is active.
        let (h, dir) = tmp_history("wal");
        let mode: String = h.0.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
        assert_eq!(mode, "wal");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_fresh_database_and_its_wal_and_shm_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("monlizard-hist-modes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = dir.join("h.db");
        let mut h = History::at(&db).unwrap();
        h.record_persistence(&[Observed { path: "/x", fingerprint: "f", schedule: "" }], &["/".into()], 1).unwrap(); // a write, so the -wal exists
        for suffix in ["", "-wal", "-shm"] {
            let p = PathBuf::from(format!("{}{suffix}", db.display()));
            assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600, "{}", p.display());
        }
        drop(h);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
