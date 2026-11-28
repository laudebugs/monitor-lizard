//! What the local classifier said about a group, kept so the owner can compare it with the rules
//! before the dashboard shows it (shadow mode). Rows hold app group names and the model's rationale,
//! which say what runs on this Mac, so they are pruned by age on every write and every time the
//! database is opened (the server opens it all the time), and `agent off` stops new ones.
use super::History;
use crate::classify::Verdict;
use rusqlite::params;
use serde::Serialize;

pub(super) const VERDICTS_TABLE: &str = "CREATE TABLE IF NOT EXISTS verdicts(
    id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, group_key TEXT NOT NULL, mode TEXT NOT NULL,
    rule_label TEXT NOT NULL, label TEXT NOT NULL, confidence TEXT NOT NULL,
    actions TEXT NOT NULL, rationale TEXT NOT NULL);
    CREATE INDEX IF NOT EXISTS verdicts_ts ON verdicts(ts);";

/// A verdict older than this is deleted the next time one is recorded.
pub const VERDICTS_KEEP_SECS: u64 = 14 * 86400;

#[derive(Debug, PartialEq, Serialize)]
pub struct VerdictRow {
    pub ts: u64,
    pub group_key: String,
    pub mode: String,
    pub rule_label: String,
    pub label: String,
    pub confidence: String,
    pub actions: Vec<String>,
    pub rationale: String,
}

/// The lowercase word a plain enum serializes to (`"suspicious"`, `"force_quit"`).
fn word<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

/// Deletes the verdicts older than `VERDICTS_KEEP_SECS`. Run when the database is opened as well as
/// on every write, so rows do not outlive the keep window after the agent is turned off.
pub(super) fn prune(c: &rusqlite::Connection, now: u64) -> rusqlite::Result<()> {
    c.execute("DELETE FROM verdicts WHERE ts < ?1", params![now.saturating_sub(VERDICTS_KEEP_SECS) as i64]).map(|_| ())
}

impl History {
    /// Writes one checked verdict and deletes the ones past `VERDICTS_KEEP_SECS`. `mode` is the
    /// agent mode in force (`shadow` or `on`), so a later comparison knows whether the owner saw it.
    pub fn record_verdict(&self, ts: u64, group_key: &str, mode: &str, rule_label: crate::model::Label, v: &Verdict) -> rusqlite::Result<()> {
        let actions: Vec<String> = v.actions.iter().map(word).collect();
        self.0.execute(
            "INSERT INTO verdicts(ts,group_key,mode,rule_label,label,confidence,actions,rationale) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![ts as i64, group_key, mode, word(&rule_label), word(&v.label), word(&v.confidence), serde_json::to_string(&actions).unwrap_or_default(), v.rationale],
        )?;
        prune(&self.0, ts)
    }

    /// Newest first.
    pub fn recent_verdicts(&self, limit: usize) -> rusqlite::Result<Vec<VerdictRow>> {
        let mut st = self.0.prepare("SELECT ts,group_key,mode,rule_label,label,confidence,actions,rationale FROM verdicts ORDER BY id DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit as i64], |r| Ok(VerdictRow {
            ts: r.get::<_, i64>(0)? as u64, group_key: r.get(1)?, mode: r.get(2)?, rule_label: r.get(3)?, label: r.get(4)?,
            confidence: r.get(5)?, actions: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(), rationale: r.get(7)?,
        }))?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::Action;
    use crate::history::test_util::tmp_history;
    use crate::llm::Confidence;
    use crate::model::Label;

    fn verdict(label: Label, rationale: &str) -> Verdict {
        Verdict { label, confidence: Confidence::Medium, actions: vec![Action::Investigate, Action::ForceQuit], rationale: rationale.into() }
    }

    #[test]
    fn a_verdict_is_stored_as_words_and_read_back_newest_first() {
        let (h, dir) = tmp_history("verdict-roundtrip");
        h.record_verdict(100, "/Apps/A.app", "shadow", Label::Unknown, &verdict(Label::Suspicious, "listens on a new port")).unwrap();
        h.record_verdict(200, "/Apps/B.app", "on", Label::Trusted, &verdict(Label::Trusted, "<b>signed</b>")).unwrap();
        let rows = h.recent_verdicts(10).unwrap();
        assert_eq!(rows.iter().map(|r| r.group_key.as_str()).collect::<Vec<_>>(), ["/Apps/B.app", "/Apps/A.app"]);
        assert_eq!((rows[1].mode.as_str(), rows[1].rule_label.as_str(), rows[1].label.as_str(), rows[1].confidence.as_str()), ("shadow", "unknown", "suspicious", "medium"));
        assert_eq!(rows[1].actions, ["investigate", "force_quit"]);
        assert_eq!(rows[0].rationale, "<b>signed</b>", "markup is stored as the text it is");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn opening_the_database_deletes_old_verdicts_even_when_nothing_is_written() {
        let (h, dir) = tmp_history("verdict-open-prune");
        h.record_verdict(5, "/Apps/A.app", "shadow", Label::Unknown, &verdict(Label::Unknown, "")).unwrap(); // far older than the keep window
        drop(h);
        let again = History::at(&dir.join("h.db")).unwrap();
        assert!(again.recent_verdicts(10).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_write_deletes_verdicts_older_than_the_keep_window_and_keeps_the_boundary() {
        let (h, dir) = tmp_history("verdict-prune");
        let now = 10 * VERDICTS_KEEP_SECS;
        for ts in [now - VERDICTS_KEEP_SECS - 1, now - VERDICTS_KEEP_SECS, now - 5] {
            h.record_verdict(ts, "/Apps/A.app", "shadow", Label::Unknown, &verdict(Label::Unknown, "")).unwrap();
        }
        h.record_verdict(now, "/Apps/A.app", "shadow", Label::Unknown, &verdict(Label::Unknown, "")).unwrap();
        let mut kept: Vec<u64> = h.recent_verdicts(10).unwrap().iter().map(|r| r.ts).collect();
        kept.sort_unstable();
        assert_eq!(kept, vec![now - VERDICTS_KEEP_SECS, now - 5, now]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
