use super::{History, LEARNING_SECS};
use rusqlite::params;

/// What one scan saw of one startup item.
pub struct Observed<'a> { pub path: &'a str, pub fingerprint: &'a str, pub schedule: &'a str }

/// How long a removed item stays in the database before it is forgotten.
const REMOVED_KEEP_SECS: u64 = 90 * 86400;
/// The change list covers this long at most, and holds this many entries at most.
const CHANGES_WINDOW_SECS: u64 = 30 * 86400;
pub const MAX_CHANGES: usize = 200;

/// One stored startup item. `changed_at` is the last time its fingerprint changed, `restored_at`
/// the last time it came back after being removed; they are separate so neither hides the other.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistRow {
    pub path: String, pub first_seen: u64, pub schedule: String, pub fingerprint: Option<String>,
    pub removed_at: Option<u64>, pub changed_at: Option<u64>, pub restored_at: Option<u64>,
}

#[derive(serde::Serialize, Debug, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind { Added, Changed, Restored, Removed }

/// One entry of the "since the baseline" list.
#[derive(serde::Serialize, Debug, PartialEq, Clone)]
pub struct Change {
    pub path: String, pub kind: ChangeKind, pub at: u64,
    /// The item was removed and came back after the baseline.
    pub restored: bool,
    /// An added item was edited after it was added; `last_change_at` says when.
    pub changed_since_added: bool,
    pub last_change_at: Option<u64>,
}

/// The baseline date for startup items: the end of the learning period after the very first scan.
/// Everything recorded before it is the baseline, so a first run has no changes.
pub fn persistence_baseline(rows: &[PersistRow]) -> u64 {
    rows.iter().map(|r| r.first_seen).min().unwrap_or(0) + LEARNING_SECS
}

/// What happened to startup items since the baseline date, newest first, limited to the last 30
/// days and 200 entries; the flag says entries were left out. One entry per item: a removal wins
/// over an addition, an addition over an edit, and an edit over a return.
pub fn persistence_changes(rows: &[PersistRow], now: u64) -> (Vec<Change>, bool) {
    let base = persistence_baseline(rows).max(now.saturating_sub(CHANGES_WINDOW_SECS));
    let after = |t: Option<u64>| t.filter(|t| *t > base);
    let mut out: Vec<Change> = rows.iter().filter_map(|r| {
        let (kind, at) = if let Some(at) = after(r.removed_at) { (ChangeKind::Removed, at) }
            else if r.first_seen > base { (ChangeKind::Added, r.first_seen) }
            else if let Some(at) = after(r.changed_at) { (ChangeKind::Changed, at) }
            else if let Some(at) = after(r.restored_at) { (ChangeKind::Restored, at) }
            else { return None };
        Some(Change { path: r.path.clone(), kind, at, restored: after(r.restored_at).is_some(),
            changed_since_added: kind == ChangeKind::Added && r.changed_at.is_some(), last_change_at: r.changed_at })
    }).collect();
    out.sort_by(|a, b| b.at.cmp(&a.at).then(a.path.cmp(&b.path)));
    let truncated = out.len() > MAX_CHANGES;
    out.truncate(MAX_CHANGES);
    (out, truncated)
}

impl History {
    /// Records one startup scan and returns every row, removed ones included. `scanned` lists the path
    /// prefixes the scan could actually read: a folder that failed to open must not make its items
    /// look removed. A row is written only when something about it changed (new, fingerprint,
    /// removed, restored), so a quiet scan writes nothing.
    pub fn record_persistence(&mut self, seen: &[Observed], scanned: &[String], now: u64) -> rusqlite::Result<Vec<PersistRow>> {
        let tx = self.0.transaction()?;
        let known: std::collections::HashMap<String, (Option<String>, Option<i64>)> = {
            let mut st = tx.prepare("SELECT path, fingerprint, removed_at FROM persistence")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?))))?.collect::<Result<_, _>>()?;
            rows
        };
        let now_i = now as i64;
        for o in seen {
            match known.get(o.path) {
                None => { tx.execute("INSERT INTO persistence(path,first_seen,last_seen,fingerprint,schedule) VALUES(?1,?2,?2,?3,?4)", params![o.path, now_i, o.fingerprint, o.schedule])?; }
                // Coming back with different content is an edit as well as a return: both are recorded, so the return cannot hide the edit.
                Some((fp, Some(_))) => {
                    let edited = fp.as_deref().is_some_and(|f| f != o.fingerprint);
                    tx.execute("UPDATE persistence SET removed_at=NULL, restored_at=?4, fingerprint=?2, schedule=?3, changed_at=CASE WHEN ?5 THEN ?4 ELSE changed_at END, last_seen=?4 WHERE path=?1", params![o.path, o.fingerprint, o.schedule, now_i, edited])?;
                }
                // A row from before fingerprints existed: learn it silently, or every item would read as changed after an upgrade.
                Some((None, None)) => { tx.execute("UPDATE persistence SET fingerprint=?2, schedule=?3 WHERE path=?1", params![o.path, o.fingerprint, o.schedule])?; }
                Some((Some(fp), None)) if fp != o.fingerprint => { tx.execute("UPDATE persistence SET fingerprint=?2, schedule=?3, changed_at=?4, last_seen=?4 WHERE path=?1", params![o.path, o.fingerprint, o.schedule, now_i])?; }
                Some(_) => {}
            }
        }
        // A removed item is kept for a while so it can be shown and can return as the same row, then forgotten.
        tx.execute("DELETE FROM persistence WHERE removed_at IS NOT NULL AND removed_at < ?1", [now_i.saturating_sub(REMOVED_KEEP_SECS as i64)])?;
        let present: std::collections::HashSet<&str> = seen.iter().map(|o| o.path).collect();
        for (path, (_, removed)) in &known {
            if removed.is_none() && !present.contains(path.as_str()) && scanned.iter().any(|p| path.starts_with(p.as_str())) {
                tx.execute("UPDATE persistence SET removed_at=?2 WHERE path=?1", params![path, now_i])?;
            }
        }
        tx.commit()?;
        let mut st = self.0.prepare("SELECT path, first_seen, schedule, fingerprint, removed_at, changed_at, restored_at FROM persistence")?;
        let rows = st.query_map([], |r| Ok(PersistRow {
            path: r.get(0)?, first_seen: r.get::<_, i64>(1)? as u64, schedule: r.get(2)?, fingerprint: r.get(3)?,
            removed_at: r.get::<_, Option<i64>>(4)?.map(|v| v as u64), changed_at: r.get::<_, Option<i64>>(5)?.map(|v| v as u64), restored_at: r.get::<_, Option<i64>>(6)?.map(|v| v as u64),
        }))?.collect::<Result<_, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{migrations::MIGRATIONS, test_util::scratch};
    use rusqlite::Connection;

    fn obs<'a>(path: &'a str, fp: &'a str) -> Observed<'a> { Observed { path, fingerprint: fp, schedule: "at load" } }
    fn row<'a>(rows: &'a [PersistRow], path: &str) -> &'a PersistRow { rows.iter().find(|r| r.path == path).unwrap() }
    const ALL: &[&str] = &["/"];

    #[test]
    fn startup_items_keep_their_first_seen_time_across_removal_and_return() {
        let dir = scratch("pers-first");
        let mut h = History::at(&dir.join("h.db")).unwrap();
        let rows = h.record_persistence(&[obs("/a", "1")], &["/".into()], 1000).unwrap();
        assert_eq!(row(&rows, "/a").first_seen, 1000);
        let rows = h.record_persistence(&[obs("/a", "1"), obs("/b", "1")], &["/".into()], 1120).unwrap();
        assert_eq!((row(&rows, "/a").first_seen, row(&rows, "/b").first_seen), (1000, 1120));
        h.record_persistence(&[obs("/b", "1")], &["/".into()], 1200).unwrap();
        let rows = h.record_persistence(&[obs("/a", "1"), obs("/b", "1")], &["/".into()], 1300).unwrap();
        assert_eq!(rows.len(), 2, "removal and return keep one row");
        assert_eq!(row(&rows, "/a").first_seen, 1000);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unchanged_scan_writes_nothing() {
        let dir = scratch("pers-quiet");
        let mut h = History::at(&dir.join("h.db")).unwrap();
        h.record_persistence(&[obs("/a", "1"), obs("/b", "2")], &["/".into()], 1000).unwrap();
        let before = h.0.total_changes();
        h.record_persistence(&[obs("/a", "1"), obs("/b", "2")], &["/".into()], 1100).unwrap();
        assert_eq!(h.0.total_changes(), before, "no row was written");
        h.record_persistence(&[obs("/a", "9"), obs("/b", "2")], &["/".into()], 1200).unwrap();
        assert_eq!(h.0.total_changes(), before + 1, "a changed fingerprint writes one row");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_removed_item_gets_removed_at_and_a_returning_one_loses_it_with_a_restore_entry() {
        let dir = scratch("pers-removed");
        let mut h = History::at(&dir.join("h.db")).unwrap();
        h.record_persistence(&[obs("/a", "1"), obs("/b", "1")], &["/".into()], 1000).unwrap();
        let rows = h.record_persistence(&[obs("/b", "1")], &["/".into()], 2000).unwrap();
        assert_eq!(row(&rows, "/a").removed_at, Some(2000));
        assert_eq!(persistence_changes(&rows, 2000).0, vec![Change { path: "/a".into(), kind: ChangeKind::Removed, at: 2000, restored: false, changed_since_added: false, last_change_at: None }]);
        let rows = h.record_persistence(&[obs("/a", "1"), obs("/b", "1")], &["/".into()], 3000).unwrap();
        assert_eq!(row(&rows, "/a").removed_at, None);
        assert_eq!(rows.len(), 2);
        assert_eq!(persistence_changes(&rows, 3000).0, vec![Change { path: "/a".into(), kind: ChangeKind::Restored, at: 3000, restored: true, changed_since_added: false, last_change_at: None }]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_folder_that_could_not_be_read_does_not_remove_its_items() {
        let dir = scratch("pers-unscanned");
        let mut h = History::at(&dir.join("h.db")).unwrap();
        h.record_persistence(&[obs("/agents/a", "1"), obs("/daemons/d", "1")], &["/agents/".into(), "/daemons/".into()], 1000).unwrap();
        let rows = h.record_persistence(&[], &["/agents/".into()], 2000).unwrap();
        assert_eq!(row(&rows, "/agents/a").removed_at, Some(2000));
        assert_eq!(row(&rows, "/daemons/d").removed_at, None, "its folder was not scanned");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_change_list_reports_added_changed_and_removed_after_the_baseline_and_first_run_is_empty() {
        let dir = scratch("pers-diff");
        let mut h = History::at(&dir.join("h.db")).unwrap();
        let rows = h.record_persistence(&[obs("/a", "1"), obs("/b", "1"), obs("/c", "1")], ALL.iter().map(|s| s.to_string()).collect::<Vec<_>>().as_slice(), 1000).unwrap();
        assert!(persistence_changes(&rows, 1000).0.is_empty(), "the first scan is the baseline");
        h.record_persistence(&[obs("/a", "1"), obs("/b", "1"), obs("/c", "1")], &["/".into()], 1030).unwrap();
        let rows = h.record_persistence(&[obs("/a", "2"), obs("/c", "1"), obs("/d", "1")], &["/".into()], 5000).unwrap();
        let kinds: Vec<(String, ChangeKind)> = persistence_changes(&rows, 5000).0.into_iter().map(|c| (c.path, c.kind)).collect();
        assert_eq!(kinds.len(), 3);
        for want in [("/a", ChangeKind::Changed), ("/b", ChangeKind::Removed), ("/d", ChangeKind::Added)] { assert!(kinds.contains(&(want.0.into(), want.1)), "{want:?} in {kinds:?}"); }
        assert!(kinds.iter().all(|k| k.0 != "/c"), "an unchanged item is not listed");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn coming_back_with_different_content_records_the_edit_and_the_return() {
        let dir = scratch("pers-restore-edit");
        let mut h = History::at(&dir.join("h.db")).unwrap();
        h.record_persistence(&[obs("/a", "1"), obs("/b", "1")], &["/".into()], 1000).unwrap();
        h.record_persistence(&[obs("/b", "1")], &["/".into()], 2000).unwrap();
        let rows = h.record_persistence(&[obs("/a", "EVIL"), obs("/b", "1")], &["/".into()], 3000).unwrap();
        let c = persistence_changes(&rows, 3000).0;
        assert_eq!((c[0].kind, c[0].restored, c[0].at), (ChangeKind::Changed, true, 3000), "{c:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_added_item_that_is_edited_stays_added_and_says_it_changed() {
        let dir = scratch("pers-added-edited");
        let mut h = History::at(&dir.join("h.db")).unwrap();
        h.record_persistence(&[obs("/a", "1")], &["/".into()], 1000).unwrap();
        h.record_persistence(&[obs("/a", "1"), obs("/n", "1")], &["/".into()], 2000).unwrap();
        let rows = h.record_persistence(&[obs("/a", "1"), obs("/n", "2")], &["/".into()], 2500).unwrap();
        let c = persistence_changes(&rows, 2500).0;
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].kind, c[0].changed_since_added, c[0].last_change_at, c[0].at), (ChangeKind::Added, true, Some(2500), 2000));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_change_list_is_capped_at_200_newest_first_and_says_so() {
        let rows: Vec<PersistRow> = std::iter::once(PersistRow { path: "/old".into(), first_seen: 0, schedule: String::new(), fingerprint: None, removed_at: None, changed_at: None, restored_at: None })
            .chain((0..250u64).map(|i| PersistRow { path: format!("/n{i:03}"), first_seen: 1000 + i, schedule: String::new(), fingerprint: None, removed_at: None, changed_at: None, restored_at: None })).collect();
        let (c, truncated) = persistence_changes(&rows, 2000);
        assert_eq!((c.len(), truncated), (MAX_CHANGES, true));
        assert_eq!(c[0].path, "/n249", "newest first");
        assert!(!persistence_changes(&rows[..10], 2000).1);
    }

    #[test]
    fn changes_older_than_thirty_days_leave_the_list() {
        let day = 86400;
        let row = |path: &str, first_seen| PersistRow { path: path.into(), first_seen, schedule: String::new(), fingerprint: None, removed_at: None, changed_at: None, restored_at: None };
        let rows = vec![row("/base", 0), row("/old", 10 * day), row("/recent", 50 * day)];
        let names: Vec<String> = persistence_changes(&rows, 60 * day).0.into_iter().map(|c| c.path).collect();
        assert_eq!(names, vec!["/recent"]);
    }

    #[test]
    fn a_removed_row_is_forgotten_ninety_days_after_removal_and_not_before() {
        let dir = scratch("pers-prune");
        let mut h = History::at(&dir.join("h.db")).unwrap();
        let day = 86400;
        h.record_persistence(&[obs("/a", "1"), obs("/b", "1")], &["/".into()], 1000).unwrap();
        h.record_persistence(&[obs("/b", "1")], &["/".into()], 2000).unwrap();
        let rows = h.record_persistence(&[obs("/b", "1")], &["/".into()], 2000 + 89 * day).unwrap();
        assert!(rows.iter().any(|r| r.path == "/a"), "kept at 89 days");
        let rows = h.record_persistence(&[obs("/b", "1")], &["/".into()], 2000 + 91 * day).unwrap();
        assert!(rows.iter().all(|r| r.path != "/a"), "pruned at 91 days");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_row_from_before_fingerprints_is_learned_without_counting_as_changed() {
        let dir = scratch("pers-backfill");
        let db = dir.join("h.db");
        let c = Connection::open(&db).unwrap();
        c.execute_batch(&MIGRATIONS[..2].join("\n")).unwrap();
        c.execute_batch("INSERT INTO persistence VALUES('/a',100,200);").unwrap();
        c.pragma_update(None, "user_version", 2).unwrap();
        drop(c);
        let mut h = History::at(&db).unwrap();
        let rows = h.record_persistence(&[obs("/a", "1")], &["/".into()], 9000).unwrap();
        assert_eq!(row(&rows, "/a").fingerprint.as_deref(), Some("1"));
        assert!(persistence_changes(&rows, 9000).0.is_empty(), "the upgrade must not flag every item");
        let rows = h.record_persistence(&[obs("/a", "2")], &["/".into()], 9100).unwrap();
        assert_eq!(persistence_changes(&rows, 9100).0.len(), 1, "a real edit afterwards still shows");
        let _ = std::fs::remove_dir_all(dir);
    }
}
