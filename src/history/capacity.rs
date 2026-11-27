use super::{path, unavailable, History};
use crate::storage::forecast::{self, Day};
use rusqlite::{params, Connection};
use std::path::PathBuf;

/// Made when the database is opened, not by a migration step: nothing alters it later, so `IF NOT
/// EXISTS` is enough and `user_version` stays what the other tables expect. It holds three byte counts
/// and the day, nothing about apps or folders.
pub(super) const CAPACITY_TABLE: &str = "CREATE TABLE IF NOT EXISTS capacity_daily(day INTEGER PRIMARY KEY, ts INTEGER NOT NULL, used INTEGER NOT NULL, purgeable INTEGER NOT NULL, free INTEGER NOT NULL);";

/// How far ahead of `now` a stored row may be dated before it is treated as a clock error and deleted.
const CLOCK_SLACK_SECS: u64 = 86_400;

fn series_from(c: &Connection, now: u64) -> rusqlite::Result<Vec<Day>> {
    let mut st = match c.prepare("SELECT day, ts, used, purgeable, free FROM capacity_daily WHERE ts >= ?1 ORDER BY day") {
        Ok(st) => st,
        // A database the writer has not reached yet has no table: that is an empty series, not a failure.
        Err(rusqlite::Error::SqliteFailure(_, Some(m))) if m.contains("no such table") => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let rows = st.query_map(params![now.saturating_sub(RETENTION_SECS) as i64], |r| Ok(Day { day: r.get(0)?, ts: r.get::<_, i64>(1)? as u64, used: r.get::<_, i64>(2)? as u64, purgeable: r.get::<_, i64>(3)? as u64, free: r.get::<_, i64>(4)? as u64 }))?;
    rows.collect()
}

/// The capacity series for `GET /api/storage/history`: a read-only connection that never creates,
/// migrates or locks for writing, so a poll cannot block or be blocked by the writer. A missing file is
/// an empty series; a locked one waits up to 3 s (WAL readers do not wait on a writer at all).
pub fn read_capacity_series(p: &std::path::Path, now: u64) -> rusqlite::Result<Vec<Day>> {
    if !p.exists() { return Ok(Vec::new()); }
    let c = Connection::open_with_flags(p, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    c.busy_timeout(std::time::Duration::from_secs(3))?;
    series_from(&c, now)
}
/// The history database's path, for the read-only route.
pub fn db_path() -> PathBuf { path() }

const RETENTION_SECS: u64 = forecast::RETENTION_DAYS as u64 * 86400;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CapacityWrite { Written, AlreadyToday, ClockBehind }

/// Local calendar days since 1970-01-01 for a unix time, given the zone's offset from UTC.
pub fn local_day(ts: u64, utc_offset: i64) -> i64 { (ts as i64 + utc_offset).div_euclid(86400) }

/// The local zone's offset from UTC at `ts`, in seconds (daylight saving included). The one `unsafe`
/// call lives here, not in `iokit.rs`, because that module is for IOKit handles and this is a plain
/// libc time call that only the history needs.
pub fn local_utc_offset(ts: u64) -> i64 {
    let t = ts as libc::time_t;
    // SAFETY: `localtime_r` writes into the zeroed struct we own and reads `t`; a null result is checked.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() { 0 } else { tm.tm_gmtoff as i64 }
    }
}

/// One capacity tick: read the capacity (only when asked) and store it. Pulled out of the thread loop
/// so a test can drive it without a real clock or disk.
pub fn capacity_tick(h: &History, now: u64, offset: i64, capacity: impl FnOnce() -> Option<(u64, u64, u64)>) -> rusqlite::Result<Option<CapacityWrite>> {
    match capacity() { Some(c) => h.record_capacity(now, offset, c).map(Some), None => Ok(None) }
}

/// Logs a tick's outcome. A sample ignored because the clock is behind is said once per local day, so a
/// stuck clock is visible without filling the log every 10 minutes.
fn report_tick(outcome: &rusqlite::Result<Option<CapacityWrite>>, now: u64, offset: i64, logged: &mut (i64, i64)) {
    let day = local_day(now, offset);
    match outcome {
        Err(e) => eprintln!("monitor-lizard: could not store the capacity sample: {e}"),
        Ok(None) => if logged.0 != day { logged.0 = day; eprintln!("monitor-lizard: the disk capacity could not be read, so no capacity sample was stored"); },
        Ok(Some(CapacityWrite::ClockBehind)) => if logged.1 != day { logged.1 = day; eprintln!("monitor-lizard: the clock is behind the newest capacity sample, so today's sample was skipped"); },
        Ok(Some(_)) => {}
    }
}

/// Writes one capacity sample per local day, from the same reading the Storage page shows, with no
/// scan. Checked every 10 minutes, so the first check after local midnight stores the new day. One
/// connection is kept between checks and dropped after a failure; a panic in a check is logged and the
/// thread carries on.
pub fn start_capacity_writer() {
    std::thread::spawn(|| {
        let mut handle: Option<History> = None;
        let mut logged = (0i64, 0i64);
        loop {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let offset = local_utc_offset(now);
            let tick = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if handle.is_none() {
                    match History::open() {
                        Ok(h) => handle = Some(h),
                        Err(e) => { eprintln!("monitor-lizard: {}", unavailable(&e)); return; }
                    }
                }
                let outcome = capacity_tick(handle.as_ref().expect("opened above"), now, offset, crate::storage::capacity_triple);
                if outcome.is_err() { handle = None; }
                report_tick(&outcome, now, offset, &mut logged);
            }));
            if tick.is_err() { handle = None; eprintln!("monitor-lizard: the capacity writer panicked; it will try again in 10 minutes"); }
            std::thread::sleep(std::time::Duration::from_secs(600));
        }
    });
}

impl History {
    /// Stores today's capacity sample unless one exists for the local day. A sample is ignored while the
    /// newest stored row is dated in the future (the clock moved back a little). A row dated more than a
    /// day ahead came from a clock that was far ahead; it is deleted, so one bad reading cannot stop the
    /// series for good. Rows past the retention window are deleted on every call, whatever the outcome.
    pub fn record_capacity(&self, now: u64, utc_offset: i64, (used, purgeable, free): (u64, u64, u64)) -> rusqlite::Result<CapacityWrite> {
        let ahead = self.0.execute("DELETE FROM capacity_daily WHERE ts > ?1", params![(now + CLOCK_SLACK_SECS) as i64])?;
        if ahead > 0 { eprintln!("monitor-lizard: removed {ahead} capacity sample(s) dated more than a day in the future; the clock was ahead when they were taken"); }
        self.0.execute("DELETE FROM capacity_daily WHERE ts < ?1", params![now.saturating_sub(RETENTION_SECS) as i64])?;
        let newest: Option<i64> = self.0.query_row("SELECT MAX(ts) FROM capacity_daily", [], |r| r.get(0))?;
        if newest.is_some_and(|t| (now as i64) < t) { return Ok(CapacityWrite::ClockBehind); }
        let day = local_day(now, utc_offset);
        let n = self.0.execute("INSERT OR IGNORE INTO capacity_daily(day, ts, used, purgeable, free) VALUES(?1, ?2, ?3, ?4, ?5)", params![day, now as i64, used as i64, purgeable as i64, free as i64])?;
        Ok(if n == 1 { CapacityWrite::Written } else { CapacityWrite::AlreadyToday })
    }

    /// The samples of the last 90 days, oldest first.
    pub fn capacity_series(&self, now: u64) -> rusqlite::Result<Vec<Day>> { series_from(&self.0, now) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{migrations::MIGRATIONS, test_util::{scratch, version_of}};

    const GB: u64 = 1_000_000_000;
    const DAY: u64 = 86_400;
    fn cap_db(name: &str) -> (PathBuf, History) {
        let dir = scratch(name);
        let h = History::at(&dir.join("h.db")).unwrap();
        (dir, h)
    }
    fn cap() -> (u64, u64, u64) { (400 * GB, 50 * GB, 550 * GB) }

    #[test]
    fn the_first_sample_of_a_local_day_is_stored_and_a_second_one_that_day_is_not() {
        let (dir, h) = cap_db("cap-once");
        let t0 = 100 * DAY + 3600;
        assert_eq!(h.record_capacity(t0, 0, cap()).unwrap(), CapacityWrite::Written);
        assert_eq!(h.record_capacity(t0 + 3600, 0, (1, 1, 1)).unwrap(), CapacityWrite::AlreadyToday);
        assert_eq!(h.record_capacity(t0 + DAY, 0, cap()).unwrap(), CapacityWrite::Written);
        let s = h.capacity_series(t0 + DAY).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].used, s[0].purgeable, s[0].free), (400 * GB, 50 * GB, 550 * GB), "the first sample of the day kept its numbers");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_day_turns_over_at_local_midnight_not_at_utc_midnight() {
        let offset = -8 * 3600; // 16:00 UTC on day 100 is 08:00 local on day 100; 07:59 UTC on day 101 is 23:59 local on day 100
        assert_eq!(local_day(100 * DAY + 16 * 3600, offset), 100);
        assert_eq!(local_day(101 * DAY + 7 * 3600 + 59 * 60, offset), 100);
        assert_eq!(local_day(101 * DAY + 8 * 3600, offset), 101);
        let (dir, h) = cap_db("cap-local");
        assert_eq!(h.record_capacity(100 * DAY + 16 * 3600, offset, cap()).unwrap(), CapacityWrite::Written);
        assert_eq!(h.record_capacity(101 * DAY + 7 * 3600, offset, cap()).unwrap(), CapacityWrite::AlreadyToday, "still the same local day");
        assert_eq!(h.record_capacity(101 * DAY + 8 * 3600, offset, cap()).unwrap(), CapacityWrite::Written);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_clock_that_moves_back_a_little_skips_the_sample() {
        let (dir, h) = cap_db("cap-clock");
        h.record_capacity(100 * DAY, 0, cap()).unwrap();
        h.record_capacity(101 * DAY, 0, cap()).unwrap();
        assert_eq!(h.record_capacity(101 * DAY - 1, 0, (9, 9, 9)).unwrap(), CapacityWrite::ClockBehind);
        assert_eq!(h.record_capacity(100 * DAY + DAY / 2, 0, (9, 9, 9)).unwrap(), CapacityWrite::ClockBehind, "within a day of the newest row");
        let s = h.capacity_series(101 * DAY).unwrap();
        assert_eq!(s.iter().map(|d| d.day).collect::<Vec<_>>(), [100, 101]);
        assert!(s.iter().all(|d| d.used == 400 * GB));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_row_written_by_a_clock_a_month_ahead_is_removed_and_normal_ticks_resume() {
        let (dir, h) = cap_db("cap-ahead");
        h.record_capacity(100 * DAY, 0, cap()).unwrap();
        // The clock jumps a month ahead once, then comes back.
        assert_eq!(h.record_capacity(130 * DAY, 0, (7, 7, 7)).unwrap(), CapacityWrite::Written);
        assert_eq!(h.record_capacity(101 * DAY, 0, cap()).unwrap(), CapacityWrite::Written, "the future row no longer blocks sampling");
        let s = h.capacity_series(101 * DAY).unwrap();
        assert_eq!(s.iter().map(|d| d.day).collect::<Vec<_>>(), [100, 101]);
        assert!(s.iter().all(|d| d.used == 400 * GB), "the bad row is gone, not hidden");
        assert_eq!(h.record_capacity(102 * DAY, 0, cap()).unwrap(), CapacityWrite::Written);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rows_past_ninety_days_are_pruned_even_when_the_sample_is_skipped() {
        let (dir, h) = cap_db("cap-prune-skip");
        h.record_capacity(10 * DAY, 0, cap()).unwrap();
        h.record_capacity(200 * DAY, 0, cap()).unwrap();
        assert_eq!(h.record_capacity(200 * DAY - 100, 0, cap()).unwrap(), CapacityWrite::ClockBehind);
        let stored: i64 = h.0.query_row("SELECT COUNT(*) FROM capacity_daily", [], |r| r.get(0)).unwrap();
        assert_eq!(stored, 1, "the day-10 row was pruned although the sample was skipped");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn no_row_older_than_ninety_days_is_kept() {
        let (dir, h) = cap_db("cap-prune");
        for d in [1u64, 10, 50] { h.record_capacity(d * DAY, 0, cap()).unwrap(); }
        let now = 100 * DAY;
        h.record_capacity(now, 0, cap()).unwrap();
        let days: Vec<i64> = h.capacity_series(now).unwrap().iter().map(|d| d.day).collect();
        assert_eq!(days, [10, 50, 100], "day 1 is 99 days old and gone; day 10 is exactly 90 days old and kept");
        let stored: i64 = h.0.query_row("SELECT COUNT(*) FROM capacity_daily", [], |r| r.get(0)).unwrap();
        assert_eq!(stored, 3, "deleted from the table, not only hidden");
        // Reading also hides anything past the window, even before the next write prunes it.
        assert_eq!(h.capacity_series(now + 41 * DAY).unwrap().iter().map(|d| d.day).collect::<Vec<_>>(), [100]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn opening_the_database_makes_the_table_and_adds_no_migration_step() {
        let (dir, h) = cap_db("cap-schema");
        assert_eq!(h.capacity_series(100 * DAY).unwrap(), vec![]);
        assert_eq!(version_of(&dir.join("h.db")), MIGRATIONS.len(), "no migration step was added");
        drop(h);
        History::at(&dir.join("h.db")).unwrap(); // a second open leaves the table alone
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_table_holds_only_capacity_numbers() {
        let (dir, h) = cap_db("cap-columns");
        let cols: Vec<String> = h.0.prepare("PRAGMA table_info(capacity_daily)").unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(|c| c.unwrap()).collect();
        assert_eq!(cols, ["day", "ts", "used", "purgeable", "free"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unreadable_capacity_stores_nothing() {
        let (dir, h) = cap_db("cap-none");
        assert_eq!(capacity_tick(&h, 100 * DAY, 0, || None).unwrap(), None);
        assert!(h.capacity_series(100 * DAY).unwrap().is_empty());
        assert_eq!(capacity_tick(&h, 100 * DAY, 0, || Some(cap())).unwrap(), Some(CapacityWrite::Written));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_database_that_cannot_be_opened_becomes_an_unavailable_reply() {
        let dir = scratch("cap-blocked");
        let blocker = dir.join("not-a-folder");
        std::fs::write(&blocker, b"x").unwrap();
        let err = History::at(&blocker.join("h.db")).map(|_| ()).map_err(|e| unavailable(&e)).unwrap_err();
        let v = forecast::reply(Err(err), 100);
        assert_eq!((v["available"].clone(), v["forecast"]["status"].clone()), (false.into(), "unavailable".into()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn eight_days_of_samples_give_a_forecast_with_a_range_through_the_reply() {
        let (dir, h) = cap_db("cap-eight");
        // 600 GB used on day 92, growing 10 GB a day, on a 1000 GB disk.
        for i in 0..8u64 { h.record_capacity((92 + i) * DAY + 100, 0, ((600 + 10 * i) * GB, 0, (400 - 10 * i) * GB)).unwrap(); }
        let v = forecast::reply(h.capacity_series(99 * DAY + 100).map_err(|e| unavailable(&e)), 99);
        assert_eq!(v["forecast"]["status"], "growing");
        let expected = (1000 - 670) as f64 / 10.0; // free GB after the last sample over GB a day
        assert!((v["forecast"]["days_until_full"].as_f64().unwrap() - expected).abs() < 1e-3, "{v}");
        assert!(v["forecast"]["days_low"].is_number());
        assert_eq!(v["stale"], false);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_read_only_reader_returns_the_series_and_creates_nothing() {
        let (dir, h) = cap_db("cap-reader");
        h.record_capacity(100 * DAY, 0, cap()).unwrap();
        let db = dir.join("h.db");
        assert_eq!(read_capacity_series(&db, 100 * DAY).unwrap().len(), 1);
        // A missing file is an empty series and is not created by the read.
        let missing = dir.join("none.db");
        assert_eq!(read_capacity_series(&missing, 100 * DAY).unwrap(), vec![]);
        assert!(!missing.exists());
        // A database without the table (the writer has not run yet) is empty, and the read does not add it.
        let bare = dir.join("bare.db");
        Connection::open(&bare).unwrap().execute_batch("CREATE TABLE other(x);").unwrap();
        assert_eq!(read_capacity_series(&bare, 100 * DAY).unwrap(), vec![]);
        let tables: i64 = Connection::open(&bare).unwrap().query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='capacity_daily'", [], |r| r.get(0)).unwrap();
        assert_eq!(tables, 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_reader_is_not_blocked_by_a_writer_in_the_middle_of_a_transaction() {
        let (dir, h) = cap_db("cap-busy");
        h.record_capacity(100 * DAY, 0, cap()).unwrap();
        h.0.execute_batch("BEGIN IMMEDIATE; INSERT INTO capacity_daily VALUES(101, 101000000, 1, 1, 1);").unwrap();
        let t = std::time::Instant::now();
        let rows = read_capacity_series(&dir.join("h.db"), 100 * DAY).unwrap();
        assert!(t.elapsed() < std::time::Duration::from_secs(2), "the read did not wait for the writer");
        assert_eq!(rows.len(), 1, "only the committed row is seen");
        h.0.execute_batch("ROLLBACK").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_local_offset_is_a_real_zone_offset() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let off = local_utc_offset(now);
        assert!((-12 * 3600..=14 * 3600).contains(&off), "{off}");
        assert_eq!(off % 900, 0, "every zone is a whole number of quarter hours from UTC");
        assert_eq!(local_day(now, off), local_day(now + 1, off) - if (now as i64 + off + 1) % 86_400 == 0 { 1 } else { 0 });
    }
}
