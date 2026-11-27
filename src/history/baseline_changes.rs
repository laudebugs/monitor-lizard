use super::History;
use rusqlite::params;

/// Audit rows kept; older ones are deleted. The baseline itself is not affected.
const BASELINE_CHANGE_KEEP: i64 = 10_000;

/// One time the user marked or unmarked something as expected; `marked` is false for an unmark.
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct BaselineChangeRow { pub ts: u64, pub kind: String, pub key: String, pub marked: bool, pub note: String }

impl History {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::test_util::tmp_history;

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
        // The first BASELINE_CHANGE_KEEP rows go in as one statement: a commit apiece costs a disk sync each, which can take about 35 minutes on a
        // slow runner disk. Only the last five use record_baseline_change, so the pruning is still what is tested.
        h.0.execute("INSERT INTO baseline_changes(ts,kind,key,marked,note) WITH RECURSIVE n(x) AS (SELECT 0 UNION ALL SELECT x + 1 FROM n WHERE x < ?1 - 1) SELECT x, 'app', '/' || x || '.app', 1, '' FROM n", [BASELINE_CHANGE_KEEP]).unwrap();
        for n in BASELINE_CHANGE_KEEP as u64..(BASELINE_CHANGE_KEEP as u64 + 5) { h.record_baseline_change(n, "app", &format!("/{n}.app"), true, "").unwrap(); }
        let rows = h.recent_baseline_changes(usize::MAX >> 1).unwrap();
        assert_eq!(rows.len(), BASELINE_CHANGE_KEEP as usize);
        assert_eq!(rows[0].ts, BASELINE_CHANGE_KEEP as u64 + 4, "the newest survived");
        let _ = std::fs::remove_dir_all(dir);
    }
}
