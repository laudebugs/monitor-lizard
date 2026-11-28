//! `disk_writes_hourly`: bytes written per app group per hour, kept 7 days, and only while the
//! owner has turned `settings.disk_writes_history` on. A row holds the hour, the app group's name
//! and a byte count: never a pid, a path or a file name.
use super::{open_or_report, History};
use crate::procusage::WriteCounter;
use rusqlite::{params, Connection};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

pub(super) const DISK_WRITES_TABLE: &str = "CREATE TABLE IF NOT EXISTS disk_writes_hourly(
    hour INTEGER NOT NULL, app TEXT NOT NULL, bytes INTEGER NOT NULL, PRIMARY KEY(hour, app)) WITHOUT ROWID;";

const HOUR_SECS: u64 = 3600;
/// How long a row is kept: a week covers the habits worth looking back on, and no more.
pub const DISK_WRITES_KEEP_SECS: u64 = 7 * 86400;
/// How often the pending bytes go to disk. A minute bounds what a crash loses without writing to
/// the database on every 5 s sample.
const FLUSH_SECS: u64 = 60;
/// The page's chart: the last 24 hours, the current one included.
const CHART_HOURS: u64 = 24;

/// One stored row, as `GET /api/storage/writes` reads it back.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteRow { pub hour: u64, pub app: String, pub bytes: u64 }

/// The in-memory half of the roll-up: the last counter seen per process, and the bytes not yet
/// written to the table. Dropped whenever the setting is off, so nothing carries over.
pub struct WriteRollup {
    /// When watching began: a process that started after this has written nothing we did not see.
    since: u64,
    last: HashMap<(u32, u64), u64>,
    pending: BTreeMap<(u64, String), u64>,
    flushed_at: u64,
}

impl WriteRollup {
    pub fn new(now: u64) -> Self { Self { since: now, last: HashMap::new(), pending: BTreeMap::new(), flushed_at: now } }

    /// Adds each process's growth since its last sample to the hour `now` falls in. A process is
    /// keyed by (pid, start time), so a recycled pid starts afresh instead of being diffed against
    /// the process that had it before. A process missing from `counters` has exited: its bytes up
    /// to its last sample are already counted, and it is forgotten, so nothing counts it twice.
    pub fn observe(&mut self, counters: &[WriteCounter], now: u64) {
        let hour = now - now % HOUR_SECS;
        let mut next = HashMap::with_capacity(counters.len());
        for c in counters {
            let key = (c.pid, c.start_ticks);
            let (delta, keep) = match self.last.get(&key) {
                Some(&prev) if c.written >= prev => (c.written - prev, c.written),
                // A lifetime counter never shrinks, so a lower reading is not trusted: the sample is
                // dropped and the higher one kept, which can undercount but never counts twice.
                Some(&prev) => (0, prev),
                // Seen for the first time: one that started while we watched counts from zero;
                // an older one's earlier writes happened before watching began, so they are not ours.
                None if c.start_unix.is_some_and(|s| s >= self.since) => (c.written, c.written),
                None => (0, c.written),
            };
            if delta > 0 { *self.pending.entry((hour, c.app.clone())).or_default() += delta; }
            next.insert(key, keep);
        }
        self.last = next;
    }

    fn take_pending(&mut self) -> Vec<WriteRow> {
        std::mem::take(&mut self.pending).into_iter().map(|((hour, app), bytes)| WriteRow { hour, app, bytes }).collect()
    }

    fn restore(&mut self, rows: Vec<WriteRow>) {
        for r in rows { *self.pending.entry((r.hour, r.app)).or_default() += r.bytes; }
    }
}

impl History {
    /// Adds each row's bytes to its (hour, app) row, in one transaction.
    pub fn add_disk_writes(&mut self, rows: &[WriteRow]) -> rusqlite::Result<()> {
        let tx = self.0.transaction()?;
        for r in rows {
            tx.execute("INSERT INTO disk_writes_hourly(hour, app, bytes) VALUES(?1, ?2, ?3)
                ON CONFLICT(hour, app) DO UPDATE SET bytes = bytes + excluded.bytes", params![r.hour as i64, r.app, r.bytes.min(i64::MAX as u64) as i64])?;
        }
        tx.commit()
    }

    /// Deletes the rows older than the 7 days kept.
    pub fn prune_disk_writes(&self, now: u64) -> rusqlite::Result<usize> {
        self.0.execute("DELETE FROM disk_writes_hourly WHERE hour < ?1", params![now.saturating_sub(DISK_WRITES_KEEP_SECS) as i64])
    }

    /// Deletes every row: the Storage page's "Erase history".
    pub fn erase_disk_writes(&self) -> rusqlite::Result<usize> {
        self.0.execute("DELETE FROM disk_writes_hourly", [])
    }

    /// Rows for the hours starting at or after `since`, oldest first.
    pub fn disk_writes_since(&self, since: u64) -> rusqlite::Result<Vec<WriteRow>> { rows_since(&self.0, since) }

    /// How many rows are stored at all.
    pub fn disk_writes_count(&self) -> rusqlite::Result<u64> { row_count(&self.0) }
}

fn rows_since(c: &Connection, since: u64) -> rusqlite::Result<Vec<WriteRow>> {
    let mut st = c.prepare("SELECT hour, app, bytes FROM disk_writes_hourly WHERE hour >= ?1 ORDER BY hour, app")?;
    let rows = st.query_map(params![since as i64], |r| Ok(WriteRow { hour: r.get::<_, i64>(0)? as u64, app: r.get(1)?, bytes: r.get::<_, i64>(2)?.max(0) as u64 }))?;
    rows.collect()
}

fn row_count(c: &Connection) -> rusqlite::Result<u64> {
    c.query_row("SELECT COUNT(*) FROM disk_writes_hourly", [], |r| r.get::<_, i64>(0)).map(|n| n.max(0) as u64)
}

/// What the reply reads from the table: the rows of the last 24 hours, and how many rows are kept
/// in all, so the page can offer "Erase history" even when the last 24 hours are empty.
#[derive(Debug, Default, PartialEq)]
pub struct Stored { pub rows: Vec<WriteRow>, pub kept: u64 }

/// `GET /api/storage/writes`'s read: a read-only connection that never creates, migrates or locks
/// for writing, like `read_capacity_series`, so a poll cannot block the writer. A missing file or a
/// database from before the table existed reads as empty.
pub fn read_disk_writes(p: &std::path::Path, now: u64) -> rusqlite::Result<Stored> {
    if !p.exists() { return Ok(Stored::default()); }
    let c = Connection::open_with_flags(p, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    c.busy_timeout(std::time::Duration::from_secs(3))?;
    let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='disk_writes_hourly')", [], |r| r.get(0))?;
    if !exists { return Ok(Stored::default()); }
    Ok(Stored { rows: rows_since(&c, now.saturating_sub(CHART_HOURS * HOUR_SECS))?, kept: row_count(&c)? })
}

/// Shared by the sampler's hook and the erase route. True means an erase happened since the last
/// sample, so the bytes still pending from before it must not be written back. The hook holds the
/// lock for a whole sample, so a sample that took its pending bytes before the erase finishes
/// writing them before the route can set the flag, and the route's delete then removes them.
pub type EraseFlag = Arc<Mutex<bool>>;

/// The erase route's job: mark the erase for the hook, then delete every row. The lock is let go
/// before the database is opened, so a slow open never holds up the process sampler.
pub fn erase_disk_writes_now(flag: &Mutex<bool>) -> rusqlite::Result<usize> {
    *flag.lock().unwrap_or_else(|e| e.into_inner()) = true;
    History::open()?.erase_disk_writes()
}

/// The hook's memory between samples: the roll-up while the setting is on, and when rows were last
/// pruned, which goes on while it is off so no row outlives the 7 days.
#[derive(Default)]
struct HookState { rollup: Option<WriteRollup>, pruned_at: u64 }

/// One sample's whole job, apart from the thread, so a test can drive it with a fixed clock. Off
/// drops the in-memory roll-up and writes nothing; it still prunes, at most once an hour.
fn disk_writes_tick(state: &mut HookState, enabled: bool, counters: &[WriteCounter], now: u64, flag: &Mutex<bool>, open: impl FnOnce() -> Option<History>) {
    if !enabled {
        state.rollup = None;
        if now.saturating_sub(state.pruned_at) >= HOUR_SECS {
            // Stamped before the attempt, so a database that cannot be opened is retried hourly, not every sample.
            state.pruned_at = now;
            if let Some(h) = open() { let _ = h.prune_disk_writes(now); }
        }
        return;
    }
    let mut erased = flag.lock().unwrap_or_else(|e| e.into_inner());
    let r = state.rollup.get_or_insert_with(|| WriteRollup::new(now));
    if std::mem::take(&mut *erased) { r.pending.clear(); }
    r.observe(counters, now);
    if now.saturating_sub(r.flushed_at) < FLUSH_SECS { return; }
    r.flushed_at = now;
    let rows = r.take_pending();
    let Some(mut h) = open() else { r.restore(rows); return };
    // Bytes that could not be written stay pending for the next try rather than being lost.
    if h.add_disk_writes(&rows).is_err() { r.restore(rows); }
    if h.prune_disk_writes(now).is_ok() { state.pruned_at = now; }
}

/// The hook `procusage::start` calls after every sample. `enabled` is read on every call, so turning
/// the setting off stops the next sample from being kept.
pub fn disk_writes_hook(enabled: impl Fn() -> bool + Send + 'static, erased: EraseFlag) -> impl FnMut(&[WriteCounter]) + Send + 'static {
    let mut state = HookState::default();
    move |counters| {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        disk_writes_tick(&mut state, enabled(), counters, now, &erased, open_or_report);
    }
}

/// The body of `GET /api/storage/writes`: the setting, the last 24 hours per app (largest first,
/// each with one figure per hour), how many rows are kept in all, and the SSD's lifetime writes.
pub fn writes_reply(enabled: bool, stored: rusqlite::Result<Stored>, now: u64, lifetime: crate::sampler::Reading<u64>) -> serde_json::Value {
    let first = (now - now % HOUR_SECS).saturating_sub((CHART_HOURS - 1) * HOUR_SECS);
    let hours: Vec<u64> = (0..CHART_HOURS).map(|i| first + i * HOUR_SECS).collect();
    let (rows, kept, error) = match stored {
        Ok(Stored { rows, kept }) => (rows, kept, None),
        Err(e) => (Vec::new(), 0, Some(super::unavailable(&e))),
    };
    let mut apps: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for r in rows.into_iter().filter(|r| r.hour >= first) {
        let slot = ((r.hour - first) / HOUR_SECS) as usize;
        if slot < hours.len() { apps.entry(r.app).or_insert_with(|| vec![0; hours.len()])[slot] += r.bytes; }
    }
    let mut apps: Vec<_> = apps.into_iter().map(|(app, hourly)| (hourly.iter().sum::<u64>(), app, hourly)).collect();
    apps.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    serde_json::json!({
        "enabled": enabled,
        "available": error.is_none(),
        "error": error,
        "hours": hours,
        "apps": apps.into_iter().map(|(total, app, hourly)| serde_json::json!({ "app": app, "total": total, "hourly": hourly })).collect::<Vec<_>>(),
        "rows_kept": kept,
        "keep_days": DISK_WRITES_KEEP_SECS / 86400,
        "lifetime_written": lifetime,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::test_util::tmp_history;

    const T0: u64 = 1_800_000_000 - 1_800_000_000 % HOUR_SECS; // an hour boundary
    fn c(pid: u32, start: u64, app: &str, written: u64) -> WriteCounter {
        WriteCounter { pid, start_ticks: start, start_unix: Some(start), app: app.into(), written }
    }
    fn pending(r: &WriteRollup) -> Vec<(u64, String, u64)> { r.pending.iter().map(|((h, a), b)| (*h, a.clone(), *b)).collect() }

    #[test]
    fn roll_up_sums_every_process_of_one_app_group_into_one_row_per_hour() {
        let mut r = WriteRollup::new(T0);
        r.observe(&[c(1, 10, "Safari", 100), c(2, 10, "Safari", 1_000), c(3, 10, "Mail", 50)], T0 + 5);
        r.observe(&[c(1, 10, "Safari", 300), c(2, 10, "Safari", 1_500), c(3, 10, "Mail", 80)], T0 + 10);
        assert_eq!(pending(&r), vec![(T0, "Mail".into(), 30), (T0, "Safari".into(), 700)], "processes older than watching start from their first reading");
        r.observe(&[c(1, 10, "Safari", 400)], T0 + HOUR_SECS + 1);
        assert_eq!(pending(&r).last().unwrap(), &(T0 + HOUR_SECS, "Safari".into(), 100), "a new hour gets its own row");
    }

    #[test]
    fn a_process_that_exits_mid_hour_is_counted_once() {
        let mut r = WriteRollup::new(T0);
        r.observe(&[c(1, 10, "Xcode", 100)], T0 + 5);
        r.observe(&[c(1, 10, "Xcode", 600)], T0 + 10);
        r.observe(&[], T0 + 15); // exited
        r.observe(&[], T0 + 20);
        assert_eq!(pending(&r), vec![(T0, "Xcode".into(), 500)]);
    }

    #[test]
    fn a_process_started_while_watching_counts_its_writes_from_zero() {
        let mut r = WriteRollup::new(T0);
        r.observe(&[c(7, T0 + 3, "Notes", 4_096)], T0 + 5);
        assert_eq!(pending(&r), vec![(T0, "Notes".into(), 4_096)]);
    }

    #[test]
    fn a_counter_that_goes_backwards_drops_the_sample() {
        let mut r = WriteRollup::new(T0);
        r.observe(&[c(1, 10, "App", 1_000)], T0 + 5);
        r.observe(&[c(1, 10, "App", 400)], T0 + 10);
        assert!(pending(&r).is_empty(), "nothing counted for the lower reading");
        r.observe(&[c(1, 10, "App", 1_200)], T0 + 15);
        assert_eq!(pending(&r), vec![(T0, "App".into(), 200)], "counted from the last trusted reading, not from the dropped one");
    }

    #[test]
    fn a_reused_pid_is_a_new_process_not_mixed_into_the_old_one() {
        let mut r = WriteRollup::new(T0);
        r.observe(&[c(42, 10, "Old", 5_000)], T0 + 5);
        // Same pid, a later start time: a different process that started while watching.
        r.observe(&[c(42, T0 + 7, "New", 300)], T0 + 10);
        assert_eq!(pending(&r), vec![(T0, "New".into(), 300)], "no negative or inflated delta from the old process's counter");
    }

    #[test]
    fn setting_off_stores_nothing_and_forgets_what_it_had() {
        let (_h, dir) = tmp_history("dw-off");
        let db = dir.join("h.db");
        let erased = Mutex::new(false);
        let mut state = HookState::default();
        for i in 0..5 {
            disk_writes_tick(&mut state, false, &[c(1, T0 + 1, "App", 1_000 * (i + 1))], T0 + i * FLUSH_SECS, &erased, || History::at(&db).ok());
        }
        assert!(state.rollup.is_none());
        assert!(History::at(&db).unwrap().disk_writes_since(0).unwrap().is_empty(), "no row written while off");
        disk_writes_tick(&mut state, true, &[c(1, T0 + 1, "App", 9_000)], T0, &erased, || History::at(&db).ok());
        disk_writes_tick(&mut state, false, &[], T0 + 5, &erased, || History::at(&db).ok());
        assert!(state.rollup.is_none(), "turning it off drops the pending bytes too");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_tick_writes_the_pending_bytes_once_a_minute_and_keeps_them_if_the_database_is_missing() {
        let (_h, dir) = tmp_history("dw-flush");
        let db = dir.join("h.db");
        let erased = Mutex::new(false);
        let mut state = HookState::default();
        disk_writes_tick(&mut state, true, &[c(1, 10, "App", 100)], T0, &erased, || History::at(&db).ok());
        disk_writes_tick(&mut state, true, &[c(1, 10, "App", 250)], T0 + 5, &erased, || History::at(&db).ok());
        assert!(History::at(&db).unwrap().disk_writes_since(0).unwrap().is_empty(), "not written before the minute is up");
        disk_writes_tick(&mut state, true, &[c(1, 10, "App", 300)], T0 + FLUSH_SECS, &erased, || None);
        disk_writes_tick(&mut state, true, &[c(1, 10, "App", 310)], T0 + 2 * FLUSH_SECS, &erased, || History::at(&db).ok());
        assert_eq!(History::at(&db).unwrap().disk_writes_since(0).unwrap(), vec![WriteRow { hour: T0, app: "App".into(), bytes: 210 }]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn add_disk_writes_adds_to_an_existing_hour_rather_than_replacing_it() {
        let (mut h, dir) = tmp_history("dw-add");
        h.add_disk_writes(&[WriteRow { hour: T0, app: "A".into(), bytes: 10 }]).unwrap();
        h.add_disk_writes(&[WriteRow { hour: T0, app: "A".into(), bytes: 5 }, WriteRow { hour: T0, app: "B".into(), bytes: 1 }]).unwrap();
        assert_eq!(h.disk_writes_since(0).unwrap(), vec![WriteRow { hour: T0, app: "A".into(), bytes: 15 }, WriteRow { hour: T0, app: "B".into(), bytes: 1 }]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_keeps_seven_days_and_deletes_what_is_older() {
        let (mut h, dir) = tmp_history("dw-prune");
        let now = T0 + 10 * 86400;
        let row = |hour: u64| WriteRow { hour, app: "A".into(), bytes: 1 };
        h.add_disk_writes(&[row(now - DISK_WRITES_KEEP_SECS - HOUR_SECS), row(now - DISK_WRITES_KEEP_SECS), row(now - HOUR_SECS)]).unwrap();
        assert_eq!(h.prune_disk_writes(now).unwrap(), 1);
        assert_eq!(h.disk_writes_since(0).unwrap().iter().map(|r| r.hour).collect::<Vec<_>>(), vec![now - DISK_WRITES_KEEP_SECS, now - HOUR_SECS]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rows_past_seven_days_are_pruned_while_the_setting_is_off() {
        let (mut h, dir) = tmp_history("dw-prune-off");
        let now = T0 + 10 * 86400;
        h.add_disk_writes(&[WriteRow { hour: now - DISK_WRITES_KEEP_SECS - HOUR_SECS, app: "Old".into(), bytes: 1 }, WriteRow { hour: now - HOUR_SECS, app: "New".into(), bytes: 1 }]).unwrap();
        drop(h);
        let db = dir.join("h.db");
        let erased = Mutex::new(false);
        let mut state = HookState::default();
        disk_writes_tick(&mut state, false, &[], now, &erased, || History::at(&db).ok());
        assert_eq!(History::at(&db).unwrap().disk_writes_since(0).unwrap().iter().map(|r| r.app.as_str()).collect::<Vec<_>>(), vec!["New"]);
        let opened = std::cell::Cell::new(0);
        disk_writes_tick(&mut state, false, &[], now + 5, &erased, || { opened.set(opened.get() + 1); None });
        assert_eq!(opened.get(), 0, "while off, the database is opened at most once an hour");
        // A failed open is not retried on every sample either.
        disk_writes_tick(&mut state, false, &[], now + HOUR_SECS, &erased, || { opened.set(opened.get() + 1); None });
        disk_writes_tick(&mut state, false, &[], now + HOUR_SECS + 5, &erased, || { opened.set(opened.get() + 1); None });
        assert_eq!(opened.get(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_read_only_read_creates_nothing_and_reads_what_the_writer_stored() {
        let dir = std::env::temp_dir().join(format!("ow-test-dw-read-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = dir.join("h.db");
        assert_eq!(read_disk_writes(&db, T0).unwrap(), Stored::default());
        assert!(!db.exists(), "a read never creates the database");
        let mut h = History::at(&db).unwrap();
        h.add_disk_writes(&[WriteRow { hour: T0, app: "A".into(), bytes: 3 }, WriteRow { hour: T0 - 3 * 86400, app: "B".into(), bytes: 1 }]).unwrap();
        assert_eq!(read_disk_writes(&db, T0 + 10).unwrap(), Stored { rows: vec![WriteRow { hour: T0, app: "A".into(), bytes: 3 }], kept: 2 });
        drop(h);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn erase_removes_every_row_and_the_bytes_still_pending() {
        let (mut h, dir) = tmp_history("dw-erase");
        h.add_disk_writes(&[WriteRow { hour: T0, app: "A".into(), bytes: 1 }, WriteRow { hour: T0 - HOUR_SECS, app: "B".into(), bytes: 2 }]).unwrap();
        assert_eq!(h.erase_disk_writes().unwrap(), 2);
        assert_eq!(h.disk_writes_count().unwrap(), 0);
        drop(h);
        let db = dir.join("h.db");
        let erased = Mutex::new(false);
        let mut state = HookState::default();
        disk_writes_tick(&mut state, true, &[c(1, 10, "App", 100)], T0, &erased, || History::at(&db).ok());
        disk_writes_tick(&mut state, true, &[c(1, 10, "App", 900)], T0 + 5, &erased, || History::at(&db).ok());
        *erased.lock().unwrap() = true; // what the erase route sets alongside deleting the rows
        disk_writes_tick(&mut state, true, &[c(1, 10, "App", 950)], T0 + FLUSH_SECS, &erased, || History::at(&db).ok());
        assert_eq!(History::at(&db).unwrap().disk_writes_since(0).unwrap(), vec![WriteRow { hour: T0, app: "App".into(), bytes: 50 }], "only what came after the erase");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_reply_lays_out_24_hours_per_app_largest_first_and_never_names_a_pid() {
        let now = T0 + 1_000;
        let rows = vec![
            WriteRow { hour: T0 - 30 * HOUR_SECS, app: "TooOld".into(), bytes: 9 },
            WriteRow { hour: T0 - 23 * HOUR_SECS, app: "Mail".into(), bytes: 5 },
            WriteRow { hour: T0, app: "Mail".into(), bytes: 5 },
            WriteRow { hour: T0, app: "Xcode".into(), bytes: 100 },
        ];
        let v = writes_reply(true, Ok(Stored { rows, kept: 4 }), now, crate::sampler::Reading::Value(7));
        assert_eq!(v["hours"].as_array().unwrap().len(), 24);
        assert_eq!(v["hours"][23], T0);
        let apps = v["apps"].as_array().unwrap();
        assert_eq!(apps.iter().map(|a| a["app"].as_str().unwrap()).collect::<Vec<_>>(), vec!["Xcode", "Mail"]);
        assert_eq!(apps[1]["total"], 10);
        assert_eq!(apps[1]["hourly"][0], 5);
        assert_eq!(apps[1]["hourly"][23], 5);
        assert_eq!(v["rows_kept"], 4);
        assert_eq!(v["lifetime_written"], serde_json::json!({ "value": 7 }));
        assert!(!v.to_string().contains("pid"));
    }

    #[test]
    fn the_reply_says_when_the_history_cannot_be_read() {
        let v = writes_reply(false, Err(rusqlite::Error::InvalidQuery), T0, crate::sampler::Reading::Unavailable("no drive"));
        assert_eq!(v["available"], false);
        assert!(v["error"].as_str().unwrap().contains("history unavailable"));
        assert_eq!(v["enabled"], false);
        assert_eq!(v["lifetime_written"], serde_json::json!({ "unavailable": "no drive" }));
    }
}
