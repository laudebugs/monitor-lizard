use super::History;
use crate::model::*;
use rusqlite::{params, OptionalExtension};
use std::collections::{BTreeMap, BTreeSet};

/// Seconds after a group's first sighting during which new hosts count as "learning", not "new".
pub const LEARNING_SECS: u64 = 60;
/// How long a first-seen host stays flagged.
pub const FLAG_WINDOW_SECS: u64 = 3600;

#[derive(serde::Serialize, Debug)]
pub struct SeenRow { pub group_key: String, pub remote: String, pub first_seen: u64, pub last_seen: u64, pub hits: u64 }

impl History {
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
}
