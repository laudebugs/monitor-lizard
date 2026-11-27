use super::History;
use rusqlite::params;

#[derive(serde::Serialize, Debug)]
pub struct ActionRow { pub ts: u64, pub sig: String, pub sent: String, pub refused: String }

impl History {
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
}
