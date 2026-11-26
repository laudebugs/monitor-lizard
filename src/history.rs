use crate::model::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Seconds after a group's first sighting during which new hosts count as "learning", not "new".
pub const LEARNING_SECS: u64 = 60;
/// How long a first-seen host stays flagged.
pub const FLAG_WINDOW_SECS: u64 = 3600;

/// The bucket width rolled up from the in-memory 2 s ring into `res='1m'`, and how long a `1m` row
/// is kept before `prune_metrics` deletes it.
const ONE_MIN_BUCKET_SECS: u64 = 60;
const ONE_MIN_RETENTION_SECS: u64 = 48 * 3600;
/// The bucket width rolled up from `res='1m'` rows into `res='15m'`, and that tier's own retention.
const FIFTEEN_MIN_BUCKET_SECS: u64 = 900;
const FIFTEEN_MIN_RETENTION_SECS: u64 = 30 * 86400;

#[derive(serde::Serialize, Debug)]
pub struct SeenRow { pub group_key: String, pub remote: String, pub first_seen: u64, pub last_seen: u64, pub hits: u64 }
/// Audit rows kept; older ones are deleted. The baseline itself is not affected.
const BASELINE_CHANGE_KEEP: i64 = 10_000;

/// One time the user marked or unmarked something as expected; `marked` is false for an unmark.
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct BaselineChangeRow { pub ts: u64, pub kind: String, pub key: String, pub marked: bool, pub note: String }
#[derive(serde::Serialize, Debug)]
pub struct ActionRow { pub ts: u64, pub sig: String, pub sent: String, pub refused: String }

/// One bucket of downsampled system metrics (never per-process -- that is never written to disk by
/// design). Any field can be absent: a bucket with no underlying readings for that metric produces
/// no row for it at all, so a gap in the source data is a gap here too, never a zero.
#[derive(serde::Serialize, Debug, Default, Clone, PartialEq)]
pub struct HistoryPoint {
    pub ts: u64,
    pub cpu_avg: Option<f64>, pub cpu_max: Option<f64>,
    pub mem_avg: Option<f64>, pub mem_max: Option<f64>,
}

/// Ordered schema steps. `PRAGMA user_version` is how many have been applied. Never edit or reorder
/// a step that has shipped: add a new one at the end, because databases in the wild have already run
/// the old text. Step 1 is the schema from before versioning, written with IF NOT EXISTS so it is a
/// no-op on those databases.
const MIGRATIONS: &[&str] = &[
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
];

fn backup_path(p: &std::path::Path) -> PathBuf {
    let mut name = p.as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

fn user_version(c: &Connection) -> rusqlite::Result<usize> {
    c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).map(|v| v as usize)
}

/// A failed file operation, with the operation and the path, so the message
/// says what went wrong instead of hiding it in an unrelated error variant.
fn file_error(operation: &str, path: &std::path::Path, cause: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(format!("could not {operation} {}: {cause}", path.display()).into())
}

/// Removes its file when dropped, so a backup that fails halfway never leaves a temporary copy of
/// the database behind.
struct TempFile(PathBuf);
impl Drop for TempFile { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); } }

/// Deletes temporary backups left by a process that died mid-write. Only called while holding the
/// open lock, so no live process can be writing one.
fn remove_stale_backups(backup: &std::path::Path) {
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
fn migrate(c: &mut Connection, steps: &[&str], backup: Option<&std::path::Path>) -> rusqlite::Result<()> {
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
fn open_lock(db: &std::path::Path) -> rusqlite::Result<std::fs::File> {
    let mut name = db.as_os_str().to_owned();
    name.push(".lock");
    let path = PathBuf::from(name);
    let f = crate::paths::create_private_file(&path).map_err(|e| file_error("open the lock file for", &path, e))?;
    f.lock().map_err(|e| file_error("lock", &path, e))?;
    Ok(f)
}

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
        // WAL so a process killed mid-batch leaves the file intact (the write-ahead log is simply
        // replayed or rolled back on next open) rather than the default rollback journal's larger
        // window for a half-written page; every table shares one connection and one journal mode.
        c.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self(c))
    }

    /// Upsert startup items; returns (path -> first_seen, earliest first_seen across all items).
    pub fn record_persistence(&mut self, paths: &[&str], now: u64) -> rusqlite::Result<(std::collections::HashMap<String, u64>, u64)> {
        let tx = self.0.transaction()?;
        for path in paths {
            tx.execute("INSERT INTO persistence(path,first_seen,last_seen) VALUES(?1,?2,?2) ON CONFLICT(path) DO UPDATE SET last_seen=?2", params![path, now as i64])?;
        }
        tx.commit()?;
        let mut st = self.0.prepare("SELECT path, first_seen FROM persistence")?;
        let rows: Vec<(String, i64)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let earliest = rows.iter().map(|r| r.1).min().unwrap_or(now as i64) as u64;
        Ok((rows.into_iter().map(|(p, t)| (p, t as u64)).collect(), earliest))
    }

    /// The earliest time this group was ever seen making an outbound connection, if any.
    pub fn first_seen(&self, group_key: &str) -> rusqlite::Result<Option<u64>> {
        self.0.query_row("SELECT MIN(first_seen) FROM seen WHERE group_key=?1", [group_key], |r| r.get::<_, Option<i64>>(0))
            .map(|v| v.map(|v| v as u64))
    }

    pub fn seen_rows(&self, limit: usize) -> rusqlite::Result<Vec<SeenRow>> {
        let mut st = self.0.prepare("SELECT group_key,remote,first_seen,last_seen,hits FROM seen ORDER BY first_seen DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit as i64], |r| Ok(SeenRow { group_key: r.get(0)?, remote: r.get(1)?, first_seen: r.get::<_, i64>(2)? as u64, last_seen: r.get::<_, i64>(3)? as u64, hits: r.get::<_, i64>(4)? as u64 }))?;
        rows.collect()
    }

    pub fn recent_actions(&self, limit: usize) -> rusqlite::Result<Vec<ActionRow>> {
        let mut st = self.0.prepare("SELECT ts,sig,sent,refused FROM actions ORDER BY id DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit as i64], |r| Ok(ActionRow { ts: r.get::<_, i64>(0)? as u64, sig: r.get(1)?, sent: r.get(2)?, refused: r.get(3)? }))?;
        rows.collect()
    }

    /// Audit trail of every Stop/Quit the dashboard performed.
    pub fn record_action(&self, ts: u64, sig: &str, sent: &[u32], refused: &str) -> rusqlite::Result<()> {
        self.0.execute("INSERT INTO actions(ts,sig,sent,refused) VALUES(?1,?2,?3,?4)",
            params![ts as i64, sig, format!("{sent:?}"), refused])?;
        Ok(())
    }

    /// Audit trail of every mark or unmark as expected. The actions table holds only pids, so it cannot say this.
    /// `note` is empty for a normal change; a row with a note says the change did not happen, and why.
    /// Only the newest `BASELINE_CHANGE_KEEP` rows are kept, so repeated marking cannot grow the file without bound.
    pub fn record_baseline_change(&self, ts: u64, kind: &str, key: &str, marked: bool, note: &str) -> rusqlite::Result<()> {
        self.0.execute("INSERT INTO baseline_changes(ts,kind,key,marked,note) VALUES(?1,?2,?3,?4,?5)", params![ts as i64, kind, key, marked, note])?;
        self.0.execute("DELETE FROM baseline_changes WHERE id <= (SELECT MAX(id) FROM baseline_changes) - ?1", [BASELINE_CHANGE_KEEP])?;
        Ok(())
    }

    pub fn recent_baseline_changes(&self, limit: usize) -> rusqlite::Result<Vec<BaselineChangeRow>> {
        let mut st = self.0.prepare("SELECT ts,kind,key,marked,note FROM baseline_changes ORDER BY id DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit as i64], |r| Ok(BaselineChangeRow { ts: r.get::<_, i64>(0)? as u64, kind: r.get(1)?, key: r.get(2)?, marked: r.get(3)?, note: r.get(4)? }))?;
        rows.collect()
    }

    pub fn record_sudo(&self, e: &crate::watcher::SudoEvent) -> rusqlite::Result<()> {
        self.0.execute("INSERT INTO sudo_events(ts,pid,tool,args,chain) VALUES(?1,?2,?3,?4,?5)",
            params![e.ts as i64, e.pid as i64, e.tool, e.args, serde_json::to_string(&e.chain).unwrap_or_default()])?;
        Ok(())
    }

    pub fn recent_sudo(&self, limit: usize) -> rusqlite::Result<Vec<crate::watcher::SudoEvent>> {
        let mut st = self.0.prepare("SELECT ts,pid,tool,args,chain FROM sudo_events ORDER BY id DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit as i64], |r| Ok(crate::watcher::SudoEvent {
            ts: r.get::<_, i64>(0)? as u64, pid: r.get::<_, i64>(1)? as u32, tool: r.get(2)?, args: r.get(3)?,
            chain: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
        }))?;
        rows.collect()
    }

    /// Upsert every outbound remote host seen per group.
    pub fn record(&mut self, groups: &[Group], now: u64) -> rusqlite::Result<()> {
        let tx = self.0.transaction()?;
        for g in groups {
            let hosts: BTreeSet<&str> = g.conns.iter().filter(|c| c.direction == Direction::Outbound).filter_map(|c| c.remote_addr.as_deref()).collect();
            for h in hosts {
                tx.execute(
                    "INSERT INTO seen(group_key,remote,first_seen,last_seen) VALUES(?1,?2,?3,?3)
                     ON CONFLICT(group_key,remote) DO UPDATE SET last_seen=?3, hits=hits+1",
                    params![g.key, h, now as i64],
                )?;
            }
        }
        tx.commit()
    }

    /// Whether `finding_id` was notified less than `dedupe_secs` ago.
    pub fn notified_recently(&self, finding_id: &str, now: u64, dedupe_secs: u64) -> rusqlite::Result<bool> {
        let ts: Option<i64> = self.0.query_row("SELECT ts FROM notified WHERE finding_id=?1", [finding_id], |r| r.get(0)).optional()?;
        Ok(ts.is_some_and(|t| now.saturating_sub(t as u64) < dedupe_secs))
    }

    /// Records (or re-records) that `finding_id` was notified at `now`.
    pub fn mark_notified(&self, finding_id: &str, now: u64) -> rusqlite::Result<()> {
        self.0.execute("INSERT INTO notified(finding_id,ts) VALUES(?1,?2) ON CONFLICT(finding_id) DO UPDATE SET ts=?2", params![finding_id, now as i64])?;
        Ok(())
    }

    /// group_key -> hosts first seen within the flag window, excluding the group's initial learning period.
    pub fn recent_new(&self, now: u64) -> rusqlite::Result<BTreeMap<String, Vec<String>>> {
        let mut st = self.0.prepare(
            "SELECT s.group_key, s.remote FROM seen s
             JOIN (SELECT group_key, MIN(first_seen) m FROM seen GROUP BY group_key) g USING(group_key)
             WHERE s.first_seen > ?1 AND s.first_seen > g.m + ?2 ORDER BY s.first_seen DESC")?;
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for r in st.query_map(params![now.saturating_sub(FLAG_WINDOW_SECS) as i64, LEARNING_SECS as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (k, h) = r?;
            out.entry(k).or_default().push(h);
        }
        Ok(out)
    }

    /// Rolls up `cpu`/`mem` points not yet in `res='1m'` into that tier's avg/max rows. Resuming
    /// point is read back from the table itself (the latest `1m` row already there), not a separate
    /// watermark, so this is safe to call repeatedly, including right after a restart.
    pub fn rollup_1m(&self, cpu: &[(u64, f64)], mem: &[(u64, f64)], now: u64) -> rusqlite::Result<()> {
        let already = self.latest_ts("1m", "cpu_avg")?;
        self.insert_rollup("1m", "cpu", &rollup_buckets(cpu, ONE_MIN_BUCKET_SECS, already, now))?;
        self.insert_rollup("1m", "mem", &rollup_buckets(mem, ONE_MIN_BUCKET_SECS, already, now))?;
        Ok(())
    }

    /// Rolls up `res='1m'` averages into `res='15m'` the same way: max-of-maxima is exact; average-
    /// of-per-minute-averages is a close approximation of the true 15-minute average rather than an
    /// exact one (it would need the raw per-2s points, long pruned from this tier's own source by
    /// the time 15 minutes have passed) -- accepted here since a sparkline-scale chart over 30 days
    /// does not need per-sample precision.
    pub fn rollup_15m(&self, now: u64) -> rusqlite::Result<()> {
        let already = self.latest_ts("15m", "cpu_avg")?;
        for metric in ["cpu", "mem"] {
            let points = self.column_points("1m", &format!("{metric}_avg"))?;
            self.insert_rollup("15m", metric, &rollup_buckets(&points, FIFTEEN_MIN_BUCKET_SECS, already, now))?;
        }
        Ok(())
    }

    /// Deletes rows older than each tier's own retention window; a resolution's own row is never
    /// kept past it, so disk use stays bounded regardless of how long the monitor keeps running.
    pub fn prune_metrics(&self, now: u64) -> rusqlite::Result<()> {
        self.0.execute("DELETE FROM samples WHERE res='1m' AND ts < ?1", params![now.saturating_sub(ONE_MIN_RETENTION_SECS) as i64])?;
        self.0.execute("DELETE FROM samples WHERE res='15m' AND ts < ?1", params![now.saturating_sub(FIFTEEN_MIN_RETENTION_SECS) as i64])?;
        Ok(())
    }

    /// One `HistoryPoint` per distinct `ts` at the given resolution ("1m" or "15m"), newer than
    /// `since`, with whichever of the four metric columns have a row at that `ts` filled in.
    pub fn query_metrics(&self, res: &str, since: u64) -> rusqlite::Result<Vec<HistoryPoint>> {
        let mut st = self.0.prepare("SELECT ts, metric, v FROM samples WHERE res=?1 AND ts > ?2 ORDER BY ts")?;
        let rows = st.query_map(params![res, since as i64], |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?, r.get::<_, f64>(2)?)))?;
        let mut by_ts: BTreeMap<u64, HistoryPoint> = BTreeMap::new();
        for row in rows {
            let (ts, metric, v) = row?;
            let p = by_ts.entry(ts).or_insert_with(|| HistoryPoint { ts, ..Default::default() });
            match metric.as_str() {
                "cpu_avg" => p.cpu_avg = Some(v), "cpu_max" => p.cpu_max = Some(v),
                "mem_avg" => p.mem_avg = Some(v), "mem_max" => p.mem_max = Some(v),
                _ => {}
            }
        }
        Ok(by_ts.into_values().collect())
    }

    /// `None` when nothing has been rolled up yet -- distinct from `Some(0)`, a real rolled-up
    /// bucket starting at ts=0, which a plain `0` sentinel would have been indistinguishable from.
    fn latest_ts(&self, res: &str, metric: &str) -> rusqlite::Result<Option<u64>> {
        self.0.query_row("SELECT MAX(ts) FROM samples WHERE res=?1 AND metric=?2", params![res, metric], |r| r.get::<_, Option<i64>>(0)).map(|v| v.map(|v| v as u64))
    }

    fn column_points(&self, res: &str, metric: &str) -> rusqlite::Result<Vec<(u64, f64)>> {
        let mut st = self.0.prepare("SELECT ts, v FROM samples WHERE res=?1 AND metric=?2 ORDER BY ts")?;
        let rows = st.query_map(params![res, metric], |r| Ok((r.get::<_, i64>(0)? as u64, r.get(1)?)))?;
        rows.collect()
    }

    /// `INSERT OR IGNORE`: a bucket is only ever computed once its window has fully elapsed, so a
    /// row this call would collide with is always one already finalized by an earlier call, never
    /// one worth overwriting.
    fn insert_rollup(&self, res: &str, metric: &str, rows: &[(u64, f64, f64)]) -> rusqlite::Result<()> {
        for (ts, avg, max) in rows {
            self.0.execute("INSERT OR IGNORE INTO samples(res,metric,ts,v) VALUES(?1,?2,?3,?4)", params![res, format!("{metric}_avg"), *ts as i64, avg])?;
            self.0.execute("INSERT OR IGNORE INTO samples(res,metric,ts,v) VALUES(?1,?2,?3,?4)", params![res, format!("{metric}_max"), *ts as i64, max])?;
        }
        Ok(())
    }
}

/// Groups `points` into fixed `bucket_secs`-wide buckets and averages/maxes each one, but only for a
/// bucket that is both new (`bucket_start > already_rolled_up_to`, or any bucket at all when
/// nothing has been rolled up yet) and fully elapsed (`bucket_start + bucket_secs <=
/// complete_before`) -- a still-filling bucket is never rolled up early, and a bucket with zero
/// points in it is simply absent from the result, not a zero or an interpolated value.
fn rollup_buckets(points: &[(u64, f64)], bucket_secs: u64, already_rolled_up_to: Option<u64>, complete_before: u64) -> Vec<(u64, f64, f64)> {
    let mut buckets: BTreeMap<u64, Vec<f64>> = BTreeMap::new();
    for &(ts, v) in points {
        let b = ts - (ts % bucket_secs);
        if already_rolled_up_to.is_none_or(|a| b > a) && b + bucket_secs <= complete_before {
            buckets.entry(b).or_default().push(v);
        }
    }
    buckets.into_iter().map(|(b, vs)| {
        let avg = vs.iter().sum::<f64>() / vs.len() as f64;
        let max = vs.iter().cloned().fold(f64::MIN, f64::max);
        (b, avg, max)
    }).collect()
}

/// The background writer: every 30 s, if `enabled()` says history is turned on, reads whatever new
/// raw samples have landed in the in-memory ring since the last cycle, folds them into an in-memory
/// (never written to disk raw) backlog of not-yet-rolled-up points, rolls up and prunes. `enabled`
/// is re-read every cycle rather than cached, so turning the setting off takes effect on the very
/// next tick, and turning it off and back on later simply resumes (`rollup_1m` is idempotent).
/// One writer cycle's whole job, pulled out of the thread loop below so it can be tested without a
/// thread or a real 30 s sleep: when disabled, touches neither the ring nor the database at all.
fn metrics_tick(h: &History, ring: &crate::sampler::Ring, enabled: bool, last_read: &mut u64, cpu_backlog: &mut Vec<(u64, f64)>, mem_backlog: &mut Vec<(u64, f64)>, now: u64) {
    if !enabled { return; }
    for s in crate::sampler::since(ring, *last_read) {
        *last_read = s.ts;
        if let Some(crate::sampler::Reading::Value(v)) = &s.cpu.total_pct { cpu_backlog.push((s.ts, *v as f64)); }
        if let (crate::sampler::Reading::Value(a), crate::sampler::Reading::Value(w), crate::sampler::Reading::Value(c)) =
            (&s.memory.app_bytes, &s.memory.wired_bytes, &s.memory.compressed_bytes)
        { mem_backlog.push((s.ts, (*a + *w + *c) as f64)); }
    }
    let _ = h.rollup_1m(cpu_backlog, mem_backlog, now);
    let _ = h.rollup_15m(now);
    let _ = h.prune_metrics(now);
    // Points whose minute has fully elapsed are now either rolled up or, for a metric that had no
    // reading that minute, never will be -- either way there is nothing further this backlog can do
    // for them, so this is pruned by age, not by tracking which ones "hit".
    cpu_backlog.retain(|&(ts, _)| ts + ONE_MIN_BUCKET_SECS > now);
    mem_backlog.retain(|&(ts, _)| ts + ONE_MIN_BUCKET_SECS > now);
}

pub fn start_metrics_writer(ring: crate::sampler::Ring, enabled: impl Fn() -> bool + Send + 'static) {
    std::thread::spawn(move || {
        let mut last_read = 0u64;
        let mut cpu_backlog: Vec<(u64, f64)> = Vec::new();
        let mut mem_backlog: Vec<(u64, f64)> = Vec::new();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            if !enabled() { continue; } // skip opening the database at all while the setting is off
            let Some(h) = open_or_report() else { continue };
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            metrics_tick(&h, &ring, true, &mut last_read, &mut cpu_backlog, &mut mem_backlog, now);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn group(key: &str, remote: &str) -> Group {
        Group { key: key.into(), label: key.into(), category: "Apps".into(), procs: vec![], signing: None, approved: true, trust: Label::Unknown, rules_fired: vec![], expected_hosts: vec![],
            conns: vec![Conn { pid: 1, name: "x".into(), proto: "TCP".into(), state: "ESTABLISHED".into(), local_addr: "10.0.0.2".into(), local_port: 1,
                remote_addr: Some(remote.into()), remote_port: Some(443), direction: Direction::Outbound, scope: Scope::Public, exposed: false }] }
    }
    #[test]
    fn startup_items_keep_their_first_seen_time() {
        let dir = std::env::temp_dir().join(format!("ow-test-pers-{}", std::process::id()));
        let mut h = History::at(&dir.join("h.db")).unwrap();
        let (first, earliest) = h.record_persistence(&["a"], 1000).unwrap();
        assert_eq!((first["a"], earliest), (1000, 1000));
        let (first, earliest) = h.record_persistence(&["a", "b"], 1120).unwrap();
        assert_eq!((first["a"], first["b"], earliest), (1000, 1120, 1000));
        // an item that disappears and comes back keeps its original time
        h.record_persistence(&["b"], 1200).unwrap();
        let (first, _) = h.record_persistence(&["a", "b"], 1300).unwrap();
        assert_eq!(first["a"], 1000);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn flags_new_host_only_after_learning_period() {
        let dir = std::env::temp_dir().join(format!("ow-test-{}", std::process::id()));
        let mut h = History::at(&dir.join("h.db")).unwrap();
        h.record(&[group("app", "1.1.1.1")], 1000).unwrap();
        h.record(&[group("app", "2.2.2.2")], 1010).unwrap(); // within learning window
        h.record(&[group("app", "3.3.3.3")], 2000).unwrap(); // genuinely new
        let n = h.recent_new(2100).unwrap();
        assert_eq!(n["app"], vec!["3.3.3.3".to_string()]);
        assert!(h.recent_new(2000 + FLAG_WINDOW_SECS + 1).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn first_seen_is_the_earliest_outbound_sighting_or_none_for_a_group_never_seen() {
        let dir = std::env::temp_dir().join(format!("ow-test-fs-{}", std::process::id()));
        let mut h = History::at(&dir.join("h.db")).unwrap();
        h.record(&[group("app", "1.1.1.1")], 1000).unwrap();
        h.record(&[group("app", "2.2.2.2")], 1500).unwrap();
        assert_eq!(h.first_seen("app").unwrap(), Some(1000));
        assert_eq!(h.first_seen("never-seen").unwrap(), None);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn notified_recently_tracks_dedupe_by_finding_id() {
        let dir = std::env::temp_dir().join(format!("ow-test-notif-{}", std::process::id()));
        let h = History::at(&dir.join("h.db")).unwrap();
        assert!(!h.notified_recently("k:rule", 1000, 86400).unwrap(), "never notified before");
        h.mark_notified("k:rule", 1000).unwrap();
        assert!(h.notified_recently("k:rule", 1000 + 100, 86400).unwrap(), "the same finding again soon after");
        assert!(!h.notified_recently("k:rule", 1000 + 86400 + 1, 86400).unwrap(), "25 hours later: notified again");
        assert!(!h.notified_recently("other:rule", 1000 + 100, 86400).unwrap(), "a different finding is unaffected");
        h.mark_notified("k:rule", 1000 + 86400 + 1).unwrap(); // re-marking an existing row updates it, not errors
        assert!(h.notified_recently("k:rule", 1000 + 86400 + 50, 86400).unwrap());
        let _ = std::fs::remove_dir_all(dir);
    }

    fn version_of(path: &std::path::Path) -> usize {
        user_version(&Connection::open(path).unwrap()).unwrap()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ow-test-migrate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

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

    #[test]
    fn four_threads_opening_at_once_all_succeed_and_the_schema_is_applied_once() {
        let dir = scratch("race");
        let db = dir.join("h.db");
        write_unversioned_database(&db);
        let threads: Vec<_> = (0..4).map(|_| { let db = db.clone(); std::thread::spawn(move || History::at(&db).map(drop)) }).collect();
        for t in threads { t.join().unwrap().expect("every opener succeeds"); }
        assert_eq!(version_of(&db), MIGRATIONS.len());
        let _ = std::fs::remove_dir_all(dir);
    }


    /// Child half of the two-process test: waits for the start file, then opens the database.
    #[test]
    fn child_open_for_the_two_process_test() {
        let (Ok(db), Ok(go)) = (std::env::var("ML_TEST_DB"), std::env::var("ML_TEST_GO")) else { return };
        while !std::path::Path::new(&go).exists() { std::thread::sleep(std::time::Duration::from_millis(1)); }
        History::at(std::path::Path::new(&db)).expect("child opens the database");
    }

    #[test]
    fn two_real_processes_opening_at_once_make_one_backup_of_the_old_database() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("two-procs");
        let db = dir.join("h.db");
        write_unversioned_database(&db);
        let go = dir.join("go");
        let exe = std::env::current_exe().unwrap();
        let children: Vec<_> = (0..2).map(|_| std::process::Command::new(&exe)
            .args(["--exact", "history::tests::child_open_for_the_two_process_test", "--test-threads=1"])
            .env("ML_TEST_DB", &db).env("ML_TEST_GO", &go).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap()).collect();
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::fs::write(&go, "").unwrap();
        for mut c in children { assert!(c.wait().unwrap().success(), "both processes open the database"); }
        assert_eq!(version_of(&db), MIGRATIONS.len());
        let bak = Connection::open(backup_path(&db)).unwrap();
        assert_eq!(user_version(&bak).unwrap(), 0, "the backup is the pre-migration file");
        assert_eq!(bak.query_row("SELECT hits FROM seen", [], |r| r.get::<_, i64>(0)).unwrap(), 7);
        assert_eq!(std::fs::metadata(backup_path(&db)).unwrap().permissions().mode() & 0o777, 0o600);
        let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
        assert!(leftovers.is_empty(), "no temporary backup remains");
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
        assert!(second.try_lock().is_ok());
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
    fn baseline_changes_are_recorded_newest_first_with_their_direction_and_note() {
        let (h, dir) = tmp_history("baseline-changes");
        h.record_baseline_change(10, "host", "k|1.2.3.4", true, "").unwrap();
        h.record_baseline_change(20, "host", "k|1.2.3.4", false, "the change was not saved: disk full").unwrap();
        let rows = h.recent_baseline_changes(10).unwrap();
        assert_eq!(rows, vec![
            BaselineChangeRow { ts: 20, kind: "host".into(), key: "k|1.2.3.4".into(), marked: false, note: "the change was not saved: disk full".into() },
            BaselineChangeRow { ts: 10, kind: "host".into(), key: "k|1.2.3.4".into(), marked: true, note: String::new() }]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_audit_keeps_only_the_newest_rows() {
        let (h, dir) = tmp_history("baseline-prune");
        for n in 0..(BASELINE_CHANGE_KEEP as u64 + 5) { h.record_baseline_change(n, "app", &format!("/{n}.app"), true, "").unwrap(); }
        let rows = h.recent_baseline_changes(usize::MAX >> 1).unwrap();
        assert_eq!(rows.len(), BASELINE_CHANGE_KEEP as usize);
        assert_eq!(rows[0].ts, BASELINE_CHANGE_KEEP as u64 + 4, "the newest survived");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_version_1_database_gains_the_baseline_table_at_version_2_and_keeps_every_row() {
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
        assert_eq!(version_of(&db), 2);
        assert!(!backup_path(&db).exists(), "a versioned database is not backed up again");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_fresh_database_has_the_baseline_table_at_the_latest_version() {
        let dir = scratch("fresh-v2");
        let db = dir.join("h.db");
        let h = History::at(&db).unwrap();
        assert!(h.recent_baseline_changes(1).unwrap().is_empty());
        drop(h);
        assert_eq!(version_of(&db), 2);
        assert_eq!(MIGRATIONS.len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_version_2_database_opens_unchanged_and_keeps_its_audit_rows() {
        let dir = scratch("v2-noop");
        let db = dir.join("h.db");
        let h = History::at(&db).unwrap();
        h.record_baseline_change(7, "host", "k|1.1.1.1", true, "").unwrap();
        drop(h);
        let h = History::at(&db).unwrap();
        assert_eq!(h.recent_baseline_changes(10).unwrap().len(), 1);
        drop(h);
        assert_eq!(version_of(&db), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    fn tmp_history(name: &str) -> (History, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("ow-test-metrics-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        (History::at(&dir.join("h.db")).unwrap(), dir)
    }

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
    fn rollup_buckets_across_a_gap_leaves_the_gap_a_gap_not_interpolated() {
        // Minute 0 and minute 2 have points; minute 1 has none at all.
        let points = vec![(0, 10.0), (30, 20.0), (120, 40.0), (140, 60.0)];
        let rolled = rollup_buckets(&points, 60, None, 10_000);
        let bucket_starts: Vec<u64> = rolled.iter().map(|(b, _, _)| *b).collect();
        assert_eq!(bucket_starts, vec![0, 120], "minute 1 (bucket start 60) produced no row at all");
    }

    #[test]
    fn rollup_buckets_computes_average_and_maximum_per_bucket() {
        let points = vec![(0, 10.0), (30, 30.0)];
        let rolled = rollup_buckets(&points, 60, None, 10_000);
        assert_eq!(rolled, vec![(0, 20.0, 30.0)]);
    }

    #[test]
    fn rollup_buckets_never_rolls_up_a_bucket_still_filling_or_already_done() {
        let points = vec![(0, 10.0), (60, 20.0)];
        // Bucket 0 (0-60) hasn't fully elapsed yet relative to complete_before=30.
        assert!(rollup_buckets(&points, 60, None, 30).is_empty());
        // Bucket 0 has already been rolled up (Some(0) means "through bucket 0 inclusive").
        let rolled = rollup_buckets(&points, 60, Some(0), 120);
        assert_eq!(rolled.iter().map(|(b, _, _)| *b).collect::<Vec<_>>(), vec![60]);
    }

    #[test]
    fn rollup_1m_is_idempotent_and_resumes_from_what_is_already_in_the_table() {
        let (h, dir) = tmp_history("rollup1m");
        let cpu = vec![(600, 10.0), (630, 30.0)];
        let mem = vec![(600, 1000.0), (630, 2000.0)];
        h.rollup_1m(&cpu, &mem, 10_000).unwrap();
        let first = h.query_metrics("1m", 0).unwrap();
        assert_eq!(first, vec![HistoryPoint { ts: 600, cpu_avg: Some(20.0), cpu_max: Some(30.0), mem_avg: Some(1500.0), mem_max: Some(2000.0) }]);
        // Calling again with the same points (as a restarted writer resuming would) changes nothing.
        h.rollup_1m(&cpu, &mem, 10_000).unwrap();
        assert_eq!(h.query_metrics("1m", 0).unwrap(), first);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rollup_15m_rolls_up_from_the_1m_tier_not_raw_points() {
        let (h, dir) = tmp_history("rollup15m");
        for (ts, cpu, mem) in [(900u64, 10.0, 1000.0), (960, 20.0, 1000.0), (1020, 30.0, 1000.0)] {
            h.rollup_1m(&[(ts, cpu)], &[(ts, mem)], 100_000).unwrap();
        }
        h.rollup_15m(100_000).unwrap();
        let points = h.query_metrics("15m", 0).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].cpu_avg, Some(20.0));
        assert_eq!(points[0].cpu_max, Some(30.0));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_metrics_deletes_only_what_is_past_each_tiers_own_retention() {
        let (h, dir) = tmp_history("prune");
        let old_bucket = 600u64; // minute-aligned; well within the 48h window as of its own rollup time
        h.rollup_1m(&[(old_bucket, 5.0)], &[(old_bucket, 500.0)], old_bucket + 10_000).unwrap();
        let now = old_bucket + 48 * 3600 + 100; // now past the 1m tier's 48h retention for that bucket
        let recent_bucket = 48 * 3600 + 60; // minute-aligned; well within retention as of now
        h.rollup_1m(&[(recent_bucket, 9.0)], &[(recent_bucket, 900.0)], now).unwrap();
        h.prune_metrics(now).unwrap();
        let left: Vec<u64> = h.query_metrics("1m", 0).unwrap().into_iter().map(|p| p.ts).collect();
        assert_eq!(left, vec![recent_bucket], "the old bucket is gone, the recent one survives -- not every row, and not none");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn metrics_tick_touches_neither_the_ring_nor_the_database_when_disabled() {
        let (h, dir) = tmp_history("disabled");
        let ring: crate::sampler::Ring = std::sync::Arc::new(std::sync::RwLock::new(std::collections::VecDeque::new()));
        let mut last_read = 0u64;
        let mut cpu_backlog = vec![(100u64, 50.0)];
        let mut mem_backlog = vec![(100u64, 5000.0)];
        metrics_tick(&h, &ring, false, &mut last_read, &mut cpu_backlog, &mut mem_backlog, 10_000);
        assert_eq!(cpu_backlog, vec![(100, 50.0)], "backlog is untouched when disabled");
        assert_eq!(mem_backlog, vec![(100, 5000.0)]);
        assert!(h.query_metrics("1m", 0).unwrap().is_empty(), "nothing written when disabled");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_fresh_database_and_its_wal_and_shm_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("monlizard-hist-modes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = dir.join("h.db");
        let mut h = History::at(&db).unwrap();
        h.record_persistence(&["/x"], 1).unwrap(); // a write, so the -wal exists
        for suffix in ["", "-wal", "-shm"] {
            let p = PathBuf::from(format!("{}{suffix}", db.display()));
            assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600, "{}", p.display());
        }
        drop(h);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
