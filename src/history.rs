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

pub struct History(Connection);

fn path() -> PathBuf { crate::paths::data_dir().join("history.db") }

impl History {
    pub fn open() -> rusqlite::Result<Self> { Self::at(&path()) }
    pub fn at(p: &std::path::Path) -> rusqlite::Result<Self> {
        if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
        let c = Connection::open(p)?;
        // WAL so a process killed mid-batch leaves the file intact (the write-ahead log is simply
        // replayed or rolled back on next open) rather than the default rollback journal's larger
        // window for a half-written page; every table shares one connection and one journal mode.
        c.pragma_update(None, "journal_mode", "WAL")?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS seen(
            group_key TEXT NOT NULL, remote TEXT NOT NULL, first_seen INTEGER NOT NULL,
            last_seen INTEGER NOT NULL, hits INTEGER NOT NULL DEFAULT 1, PRIMARY KEY(group_key, remote));
            CREATE TABLE IF NOT EXISTS sudo_events(id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, pid INTEGER NOT NULL, tool TEXT NOT NULL, args TEXT NOT NULL, chain TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS persistence(path TEXT PRIMARY KEY, first_seen INTEGER NOT NULL, last_seen INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS actions(id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, sig TEXT NOT NULL, sent TEXT NOT NULL, refused TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS notified(finding_id TEXT PRIMARY KEY, ts INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS samples(res TEXT NOT NULL, metric TEXT NOT NULL, ts INTEGER NOT NULL, v REAL NOT NULL, PRIMARY KEY(res, metric, ts)) WITHOUT ROWID;")?;
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
            let Ok(h) = History::open() else { continue };
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            metrics_tick(&h, &ring, true, &mut last_read, &mut cpu_backlog, &mut mem_backlog, now);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn group(key: &str, remote: &str) -> Group {
        Group { key: key.into(), label: key.into(), category: "Apps".into(), procs: vec![], signing: None, approved: true, trust: Label::Unknown, rules_fired: vec![],
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
}
