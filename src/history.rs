use crate::model::*;
use rusqlite::{params, Connection};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Seconds after a group's first sighting during which new hosts count as "learning", not "new".
pub const LEARNING_SECS: u64 = 60;
/// How long a first-seen host stays flagged.
pub const FLAG_WINDOW_SECS: u64 = 3600;

#[derive(serde::Serialize, Debug)]
pub struct SeenRow { pub group_key: String, pub remote: String, pub first_seen: u64, pub last_seen: u64, pub hits: u64 }
#[derive(serde::Serialize, Debug)]
pub struct ActionRow { pub ts: u64, pub sig: String, pub sent: String, pub refused: String }

pub struct History(Connection);

fn path() -> PathBuf { PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".ownwatch/history.db") }

impl History {
    pub fn open() -> rusqlite::Result<Self> { Self::at(&path()) }
    pub fn at(p: &std::path::Path) -> rusqlite::Result<Self> {
        if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
        let c = Connection::open(p)?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS seen(
            group_key TEXT NOT NULL, remote TEXT NOT NULL, first_seen INTEGER NOT NULL,
            last_seen INTEGER NOT NULL, hits INTEGER NOT NULL DEFAULT 1, PRIMARY KEY(group_key, remote));
            CREATE TABLE IF NOT EXISTS sudo_events(id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, pid INTEGER NOT NULL, tool TEXT NOT NULL, args TEXT NOT NULL, chain TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS persistence(path TEXT PRIMARY KEY, first_seen INTEGER NOT NULL, last_seen INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS actions(id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, sig TEXT NOT NULL, sent TEXT NOT NULL, refused TEXT NOT NULL);")?;
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
        Group { key: key.into(), label: key.into(), category: "Apps".into(), procs: vec![], signing: None, approved: true,
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
}
