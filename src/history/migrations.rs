use rusqlite::Connection;
use std::path::PathBuf;

/// Ordered schema steps. `PRAGMA user_version` is how many have been applied. Never edit or reorder
/// a step that has shipped: add a new one at the end, because databases in the wild have already run
/// the old text. Step 1 is the schema from before versioning, written with IF NOT EXISTS so it is a
/// no-op on those databases.
pub(super) const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS seen(
    group_key TEXT NOT NULL, remote TEXT NOT NULL, first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL, hits INTEGER NOT NULL DEFAULT 1, PRIMARY KEY(group_key, remote));
    CREATE TABLE IF NOT EXISTS sudo_events(id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, pid INTEGER NOT NULL, tool TEXT NOT NULL, args TEXT NOT NULL, chain TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS persistence(path TEXT PRIMARY KEY, first_seen INTEGER NOT NULL, last_seen INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS actions(id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, sig TEXT NOT NULL, sent TEXT NOT NULL, refused TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS notified(finding_id TEXT PRIMARY KEY, ts INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS samples(res TEXT NOT NULL, metric TEXT NOT NULL, ts INTEGER NOT NULL, v REAL NOT NULL, PRIMARY KEY(res, metric, ts)) WITHOUT ROWID;",
    // IF NOT EXISTS so a development database that already has the table is safe.
    "CREATE TABLE IF NOT EXISTS baseline_changes(id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, kind TEXT NOT NULL, key TEXT NOT NULL, marked INTEGER NOT NULL, note TEXT NOT NULL DEFAULT '');",
    // Startup-item fingerprint, schedule and removal. The columns are nullable (or defaulted) so the
    // rows of an older database stay valid; a NULL fingerprint is filled in by the next scan without
    // counting as a change. All five run in this one step's transaction.
    "ALTER TABLE persistence ADD COLUMN fingerprint TEXT;
    ALTER TABLE persistence ADD COLUMN removed_at INTEGER;
    ALTER TABLE persistence ADD COLUMN schedule TEXT NOT NULL DEFAULT '';
    ALTER TABLE persistence ADD COLUMN changed_at INTEGER;
    ALTER TABLE persistence ADD COLUMN restored_at INTEGER;",
];

pub(super) fn backup_path(p: &std::path::Path) -> PathBuf {
    let mut name = p.as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

pub(super) fn user_version(c: &Connection) -> rusqlite::Result<usize> {
    c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).map(|v| v as usize)
}

/// A failed file operation, with the operation and the path, so the message
/// says what went wrong instead of hiding it in an unrelated error variant.
pub(super) fn file_error(operation: &str, path: &std::path::Path, cause: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(format!("could not {operation} {}: {cause}", path.display()).into())
}

/// Removes its file when dropped, so a backup that fails halfway never leaves a temporary copy of
/// the database behind.
struct TempFile(PathBuf);
impl Drop for TempFile { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }

/// Deletes temporary backups left by a process that died mid-write. Only called while holding the
/// open lock, so no live process can be writing one.
pub(super) fn remove_stale_backups(backup: &std::path::Path) {
    let (Some(dir), Some(name)) = (backup.parent(), backup.file_name().and_then(|n| n.to_str())) else { return };
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let f = e.file_name();
        let f = f.to_string_lossy();
        if f.starts_with(&format!("{name}.")) && f.ends_with(".tmp") { let _ = std::fs::remove_file(e.path()); }
    }
}

/// Writes a copy of the database to `backup`, owner-only, and checks it holds the version that was
/// read before anything changed. The caller holds the open lock, so the copy cannot be of a database
/// another process has already migrated.
fn write_backup(c: &Connection, backup: &std::path::Path, version_before: usize) -> rusqlite::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut name = backup.as_os_str().to_owned();
    name.push(format!(".{}.tmp", std::process::id()));
    let tmp = TempFile(PathBuf::from(name));
    // VACUUM INTO accepts an existing empty file. Creating it first with mode 0600 keeps the copy of
    // the sudo-launch history private; letting SQLite create it would apply the umask (usually 0644).
    crate::paths::create_new_private_file(&tmp.0).map_err(|e| file_error("create the backup file", &tmp.0, e))?;
    c.execute("VACUUM INTO ?1", [tmp.0.to_string_lossy().as_ref()])?;
    std::fs::set_permissions(&tmp.0, std::fs::Permissions::from_mode(crate::paths::FILE_MODE)).map_err(|e| file_error("make the backup owner-only", &tmp.0, e))?;
    let copy = Connection::open_with_flags(&tmp.0, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| file_error("reopen the backup", &tmp.0, e))?;
    let copied = user_version(&copy).map_err(|e| file_error("read the version of the backup", &tmp.0, e))?;
    drop(copy);
    if copied != version_before { return Err(file_error("verify the backup", &tmp.0, format!("it is at schema version {copied}, expected {version_before}"))); }
    std::fs::rename(&tmp.0, backup).map_err(|e| file_error("move the backup into place at", backup, e))
}

/// Brings the database up to `steps.len()`. Refuses a database whose version is higher: it was
/// written by a newer build, and running older code against it could corrupt it silently.
pub(super) fn migrate(c: &mut Connection, steps: &[&str], backup: Option<&std::path::Path>) -> rusqlite::Result<()> {
    let found = user_version(c)?;
    if found > steps.len() {
        return Err(rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_SCHEMA), Some(format!(
            "the history database is schema version {found} but this build only understands up to {}; \
             it was made by a newer version, so it was left unchanged (update this program, or move the file aside)",
            steps.len()))));
    }
    if found == steps.len() { return Ok(()); }
    // A file that already holds tables but was never versioned predates this mechanism: keep one
    // copy of it, as it was, before anything is changed. VACUUM INTO makes a consistent copy even
    // when part of the data still sits in the write-ahead log, which a plain file copy would miss.
    if found == 0 {
        if let Some(b) = backup {
            let has_tables: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table')", [], |r| r.get(0))?;
            if has_tables && !b.exists() { write_backup(c, b, found)?; }
        }
    }
    for (i, sql) in steps.iter().enumerate().skip(found) {
        // IMMEDIATE takes the write lock up front, so two processes cannot both run the same step.
        let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // The other process may have applied this step while we waited for the lock.
        if user_version(&tx)? > i { continue; }
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

/// Takes the exclusive lock file that sits next to the database. The lock is released when the
/// returned file is dropped, or when the process dies.
pub(super) fn open_lock(db: &std::path::Path) -> rusqlite::Result<std::fs::File> {
    let mut name = db.as_os_str().to_owned();
    name.push(".lock");
    let path = PathBuf::from(name);
    let f = crate::paths::create_private_file(&path).map_err(|e| file_error("open the lock file for", &path, e))?;
    f.lock().map_err(|e| file_error("lock", &path, e))?;
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{test_util::{scratch, version_of}, unavailable, History};

    /// The five tables as they were before versioning and before the metrics tier existed.
    fn write_unversioned_database(path: &std::path::Path) {
        let c = Connection::open(path).unwrap();
        c.pragma_update(None, "journal_mode", "WAL").unwrap();
        c.execute_batch("CREATE TABLE seen(group_key TEXT NOT NULL, remote TEXT NOT NULL, first_seen INTEGER NOT NULL,
            last_seen INTEGER NOT NULL, hits INTEGER NOT NULL DEFAULT 1, PRIMARY KEY(group_key, remote));
            CREATE TABLE sudo_events(id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, pid INTEGER NOT NULL, tool TEXT NOT NULL, args TEXT NOT NULL, chain TEXT NOT NULL);
            CREATE TABLE persistence(path TEXT PRIMARY KEY, first_seen INTEGER NOT NULL, last_seen INTEGER NOT NULL);
            CREATE TABLE actions(id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, sig TEXT NOT NULL, sent TEXT NOT NULL, refused TEXT NOT NULL);
            CREATE TABLE notified(finding_id TEXT PRIMARY KEY, ts INTEGER NOT NULL);
            INSERT INTO seen VALUES('app:x','1.2.3.4:443',100,200,7);
            INSERT INTO persistence VALUES('/Library/LaunchAgents/a.plist',100,200);
            INSERT INTO actions(ts,sig,sent,refused) VALUES(5,'TERM','[1]','');").unwrap();
    }

    #[test]
    fn a_fresh_database_runs_every_migration_and_makes_no_backup() {
        let dir = scratch("fresh");
        let db = dir.join("h.db");
        drop(History::at(&db).unwrap());
        assert_eq!(version_of(&db), MIGRATIONS.len());
        assert!(!backup_path(&db).exists(), "nothing existed to back up");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unversioned_database_keeps_every_row_reaches_the_latest_version_and_is_backed_up() {
        let dir = scratch("old");
        let db = dir.join("h.db");
        write_unversioned_database(&db);
        let h = History::at(&db).unwrap();
        assert_eq!(h.seen_rows(10).unwrap().len(), 1);
        assert_eq!(h.seen_rows(10).unwrap()[0].hits, 7);
        assert_eq!(h.recent_actions(10).unwrap().len(), 1);
        assert_eq!(h.0.query_row("SELECT COUNT(*) FROM persistence", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        // The table added after the file was written is created by step 1.
        h.0.execute("INSERT INTO samples VALUES('1m','cpu_avg',60,1.0)", []).unwrap();
        drop(h);
        assert_eq!(version_of(&db), MIGRATIONS.len());
        let bak = Connection::open(backup_path(&db)).unwrap();
        assert_eq!(bak.query_row("SELECT COUNT(*) FROM seen", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        assert_eq!(user_version(&bak).unwrap(), 0, "the backup is the file as it was");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reopening_neither_remigrates_nor_replaces_the_backup() {
        let dir = scratch("reopen");
        let db = dir.join("h.db");
        write_unversioned_database(&db);
        drop(History::at(&db).unwrap());
        let first = std::fs::metadata(backup_path(&db)).unwrap().modified().unwrap();
        // A write after the first open must not reach the backup on a later open.
        let h = History::at(&db).unwrap();
        h.record_action(9, "KILL", &[2], "").unwrap();
        drop(h);
        assert_eq!(std::fs::metadata(backup_path(&db)).unwrap().modified().unwrap(), first);
        let bak = Connection::open(backup_path(&db)).unwrap();
        assert_eq!(bak.query_row("SELECT COUNT(*) FROM actions", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        assert_eq!(version_of(&db), MIGRATIONS.len());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused_and_left_untouched() {
        let dir = scratch("newer");
        let db = dir.join("h.db");
        let c = Connection::open(&db).unwrap();
        c.execute_batch("CREATE TABLE future(x); INSERT INTO future VALUES(1);").unwrap();
        c.pragma_update(None, "user_version", (MIGRATIONS.len() + 5) as i64).unwrap();
        drop(c);
        let err = History::at(&db).err().expect("must refuse").to_string();
        assert!(err.contains("newer version") && err.contains(&(MIGRATIONS.len() + 5).to_string()), "message was: {err}");
        assert_eq!(version_of(&db), MIGRATIONS.len() + 5);
        let c = Connection::open(&db).unwrap();
        let tables: i64 = c.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table'", [], |r| r.get(0)).unwrap();
        assert_eq!(tables, 1, "no table was added");
        assert_eq!(c.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0)).unwrap(), "delete", "journal mode untouched");
        assert!(!backup_path(&db).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failing_step_rolls_back_and_keeps_the_previous_version() {
        let dir = scratch("fail");
        let db = dir.join("h.db");
        let mut c = Connection::open(&db).unwrap();
        let steps = ["CREATE TABLE a(x);", "CREATE TABLE b(x); INSERT INTO nowhere VALUES(1);", "CREATE TABLE c(x);"];
        assert!(migrate(&mut c, &steps, None).is_err());
        assert_eq!(user_version(&c).unwrap(), 1, "step 1 stays applied, step 2 does not");
        let names: Vec<String> = c.prepare("SELECT name FROM sqlite_master WHERE type='table'").unwrap()
            .query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
        assert_eq!(names, vec!["a".to_string()], "the half-run step left no table behind, and step 3 never ran");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_step_added_later_runs_once_on_a_database_already_at_the_old_version() {
        let dir = scratch("later");
        let db = dir.join("h.db");
        let mut c = Connection::open(&db).unwrap();
        migrate(&mut c, &["CREATE TABLE t(x);"], None).unwrap();
        c.execute("INSERT INTO t VALUES(1)", []).unwrap();
        let steps = ["CREATE TABLE t(x);", "ALTER TABLE t ADD COLUMN y TEXT DEFAULT 'd';"];
        migrate(&mut c, &steps, None).unwrap();
        migrate(&mut c, &steps, None).unwrap();
        assert_eq!(user_version(&c).unwrap(), 2);
        assert_eq!(c.query_row("SELECT y FROM t", [], |r| r.get::<_, String>(0)).unwrap(), "d", "the existing row kept its data");
        let _ = std::fs::remove_dir_all(dir);
    }

    // Each opener opens the lock file itself, and a flock belongs to the open file, so threads here
    // exclude each other as separate processes would. The test therefore needs no child process,
    // and it waits at most 30 s for all the threads, so a stuck opener fails instead of hanging.
    #[test]
    fn four_threads_opening_at_once_all_succeed_apply_the_schema_once_and_make_one_backup_of_the_old_database() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("race");
        let db = dir.join("h.db");
        write_unversioned_database(&db);
        let (done, finished) = std::sync::mpsc::channel();
        for _ in 0..4 { let (db, done) = (db.clone(), done.clone()); std::thread::spawn(move || { let _ = done.send(History::at(&db).map(drop)); }); }
        drop(done);
        // Without the open lock an opener fails here: threads share a pid, so two of them collide on
        // the temporary backup name, and a backup of an already-migrated file fails its version check.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        for _ in 0..4 {
            let opened = finished.recv_timeout(until.saturating_duration_since(std::time::Instant::now())).expect("the openers did not all finish within 30 s, or a thread panicked");
            opened.expect("every opener succeeds");
        }
        assert_eq!(version_of(&db), MIGRATIONS.len());
        let bak = Connection::open(backup_path(&db)).unwrap();
        assert_eq!(user_version(&bak).unwrap(), 0, "the backup is the pre-migration file");
        assert_eq!(bak.query_row("SELECT hits FROM seen", [], |r| r.get::<_, i64>(0)).unwrap(), 7);
        drop(bak);
        assert_eq!(std::fs::metadata(backup_path(&db)).unwrap().permissions().mode() & 0o777, 0o600);
        let backups: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains(".bak")).collect();
        assert_eq!(backups, vec!["h.db.bak".to_string()], "one backup and no temporary backup remains");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_open_lock_excludes_other_holders_in_the_same_process() {
        let dir = scratch("lock");
        let db = dir.join("h.db");
        let first = open_lock(&db).unwrap();
        let second = std::fs::OpenOptions::new().write(true).open(dir.join("h.db.lock")).unwrap();
        assert!(matches!(second.try_lock(), Err(std::fs::TryLockError::WouldBlock)), "a second open of the lock file must wait");
        drop(first);
        assert!(crate::paths::try_lock_within(&second, std::time::Duration::from_secs(5)), "the open lock was not released within 5 s of its holder being dropped");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_backup_is_owner_only_even_when_the_source_is_not_and_a_new_data_folder_is_0700() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("modes");
        let db = dir.join("fresh").join("h.db");
        drop(History::at(&db).unwrap());
        assert_eq!(std::fs::metadata(db.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        let old = dir.join("old.db");
        write_unversioned_database(&old);
        std::fs::set_permissions(&old, std::fs::Permissions::from_mode(0o644)).unwrap();
        drop(History::at(&old).unwrap());
        assert_eq!(std::fs::metadata(backup_path(&old)).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_stale_temporary_backup_from_a_dead_process_is_removed_on_the_next_open() {
        let dir = scratch("stale");
        let db = dir.join("h.db");
        write_unversioned_database(&db);
        let stale = dir.join("h.db.bak.99999.tmp");
        std::fs::write(&stale, "half a copy").unwrap();
        drop(History::at(&db).unwrap());
        assert!(!stale.exists());
        assert!(backup_path(&db).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_backup_stops_the_migration_leaves_no_temporary_file_and_says_what_failed() {
        let dir = scratch("backup-fails");
        let db = dir.join("h.db");
        write_unversioned_database(&db);
        let mut c = Connection::open(&db).unwrap();
        let missing = dir.join("no-such-folder").join("h.db.bak");
        let err = migrate(&mut c, MIGRATIONS, Some(&missing)).unwrap_err().to_string();
        assert!(err.contains("could not create the backup file") && err.contains("no-such-folder"), "{err}");
        assert_eq!(user_version(&c).unwrap(), 0, "nothing was migrated without a backup");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_newer_database_is_refused_with_the_data_path_in_the_message_and_stays_identical_byte_for_byte() {
        let dir = scratch("newer-bytes");
        let db = dir.join("h.db");
        let c = Connection::open(&db).unwrap();
        c.execute_batch("CREATE TABLE future(x); INSERT INTO future VALUES(1);").unwrap();
        c.pragma_update(None, "user_version", (MIGRATIONS.len() + 1) as i64).unwrap();
        drop(c);
        let before = std::fs::read(&db).unwrap();
        let e = History::at(&db).err().expect("refused");
        let text = unavailable(&e);
        assert!(text.starts_with("history unavailable: ") && text.contains("newer version"), "{text}");
        assert_eq!(std::fs::read(&db).unwrap(), before);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_version_1_database_is_brought_to_the_latest_version_and_keeps_every_row() {
        let dir = scratch("v1-to-v2");
        let db = dir.join("h.db");
        // A database made with step 1 only, with a row in every table it has.
        let c = Connection::open(&db).unwrap();
        c.execute_batch(MIGRATIONS[0]).unwrap();
        c.execute_batch("INSERT INTO seen VALUES('g','1.1.1.1',1,2,3);
            INSERT INTO sudo_events(ts,pid,tool,args,chain) VALUES(1,2,'sudo','ls','[]');
            INSERT INTO persistence VALUES('/p.plist',1,2);
            INSERT INTO actions(ts,sig,sent,refused) VALUES(1,'TERM','[1]','');
            INSERT INTO notified VALUES('k:r',1);
            INSERT INTO samples VALUES('1m','cpu_avg',60,1.5);").unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
        drop(c);
        let h = History::at(&db).unwrap();
        for t in ["seen", "sudo_events", "persistence", "actions", "notified", "samples"] {
            assert_eq!(h.0.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get::<_, i64>(0)).unwrap(), 1, "{t} kept its row");
        }
        h.record_baseline_change(1, "app", "/A.app", true, "").unwrap();
        drop(h);
        assert_eq!(version_of(&db), MIGRATIONS.len());
        assert!(!backup_path(&db).exists(), "a versioned database is not backed up again");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_fresh_database_has_the_baseline_table_and_the_persistence_columns_at_the_latest_version() {
        let dir = scratch("fresh-v2");
        let db = dir.join("h.db");
        let h = History::at(&db).unwrap();
        assert!(h.recent_baseline_changes(1).unwrap().is_empty());
        drop(h);
        assert_eq!(version_of(&db), 3);
        assert_eq!(MIGRATIONS.len(), 3);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_version_3_database_opens_unchanged_and_keeps_its_audit_rows() {
        let dir = scratch("v2-noop");
        let db = dir.join("h.db");
        let h = History::at(&db).unwrap();
        h.record_baseline_change(7, "host", "k|1.1.1.1", true, "").unwrap();
        drop(h);
        let h = History::at(&db).unwrap();
        assert_eq!(h.recent_baseline_changes(10).unwrap().len(), 1);
        drop(h);
        assert_eq!(version_of(&db), 3);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_version_2_database_gains_the_persistence_columns_at_version_3_and_keeps_every_row() {
        let dir = scratch("v2-to-v3");
        let db = dir.join("h.db");
        let c = Connection::open(&db).unwrap();
        c.execute_batch(&MIGRATIONS[..2].join("\n")).unwrap();
        c.execute_batch("INSERT INTO seen VALUES('g','1.1.1.1',1,2,3);
            INSERT INTO sudo_events(ts,pid,tool,args,chain) VALUES(1,2,'sudo','ls','[]');
            INSERT INTO persistence VALUES('/p.plist',1,2);
            INSERT INTO actions(ts,sig,sent,refused) VALUES(1,'TERM','[1]','');
            INSERT INTO notified VALUES('k:r',1);
            INSERT INTO samples VALUES('1m','cpu_avg',60,1.5);
            INSERT INTO baseline_changes(ts,kind,key,marked) VALUES(1,'app','/A.app',1);").unwrap();
        c.pragma_update(None, "user_version", 2).unwrap();
        drop(c);
        let h = History::at(&db).unwrap();
        for t in ["seen", "sudo_events", "persistence", "actions", "notified", "samples", "baseline_changes"] {
            assert_eq!(h.0.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get::<_, i64>(0)).unwrap(), 1, "{t} kept its row");
        }
        let (first, fp, removed, sched): (i64, Option<String>, Option<i64>, String) = h.0.query_row("SELECT first_seen, fingerprint, removed_at, schedule FROM persistence", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
        assert_eq!((first, fp, removed, sched.as_str()), (1, None, None, ""));
        drop(h);
        assert_eq!(version_of(&db), 3);
        assert!(!backup_path(&db).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn step_three_is_all_or_nothing() {
        let c = &mut Connection::open_in_memory().unwrap();
        migrate(c, &MIGRATIONS[..2], None).unwrap();
        // The last statement fails; the columns added before it must not survive.
        let steps = [MIGRATIONS[0], MIGRATIONS[1], "ALTER TABLE persistence ADD COLUMN fingerprint TEXT; ALTER TABLE nope ADD COLUMN x TEXT;"];
        assert!(migrate(c, &steps, None).is_err());
        assert_eq!(user_version(c).unwrap(), 2);
        assert!(c.prepare("SELECT fingerprint FROM persistence").is_err(), "the first ALTER was rolled back");
        migrate(c, MIGRATIONS, None).unwrap();
        assert_eq!(user_version(c).unwrap(), 3);
    }
}
