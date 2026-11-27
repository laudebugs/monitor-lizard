use super::History;
use rusqlite::params;

/// The audit of moves to the Trash: one row per requested item, written as `pending` before anything
/// is checked or moved, `moving` just before its move, and given its result afterwards. A row left in
/// `pending` or `moving` means the app stopped before the result was known; the item may or may not
/// have moved. `request` groups the rows of one request and is the history entry id the reply carries.
/// Full paths stay here; `/api/history` shortens them. Also made when the database is opened.
pub(super) const TRASH_ITEMS_TABLE: &str = "CREATE TABLE IF NOT EXISTS trash_items(id INTEGER PRIMARY KEY, request INTEGER NOT NULL, ts INTEGER NOT NULL, item_id TEXT NOT NULL, path TEXT NOT NULL, label TEXT NOT NULL, state TEXT NOT NULL, code TEXT, reason TEXT, bytes INTEGER);";
/// The first audit table, one row per request. Nothing writes it any more; it is still read, so a move
/// recorded before the per-item table existed stays visible.
pub(super) const TRASH_TABLE: &str = "CREATE TABLE IF NOT EXISTS trash_moves(id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, ids TEXT NOT NULL, paths TEXT NOT NULL, bytes INTEGER NOT NULL, refused TEXT NOT NULL);";
/// Item rows kept (about 200 requests of 50); older ones are deleted, as with `BASELINE_CHANGE_KEEP`.
/// Paths of removed folders are the most private thing here, so the log does not grow without bound.
pub const TRASH_KEEP: i64 = 10_000;
/// Shown for an item whose request was cut short before its result was written.
pub const INTERRUPTED_REASON: &str = "the app stopped before this was finished: it may or may not have moved";

/// One item of a request, as the audit shows it.
#[derive(serde::Serialize, Debug, PartialEq, Clone)]
pub struct TrashItemRow { pub id: String, pub path: String, pub label: String, pub state: String, pub code: Option<String>, pub reason: Option<String>, pub bytes: Option<u64> }

/// One request. `request` is None for a row from the first audit table.
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct TrashMoveRow { pub request: Option<u64>, pub ts: u64, pub bytes: u64, pub items: Vec<TrashItemRow> }

impl TrashMoveRow {
    /// The same row with the home folder written as `~`, in paths and in reason text (an OS error can
    /// quote a path), for a reply that needs no token. The full paths stay in the database.
    pub fn with_home_as_tilde(mut self, home: &std::path::Path) -> Self {
        let text = home.to_string_lossy().trim_end_matches('/').to_string();
        for i in &mut self.items {
            i.path = scrub_home(&i.path, &text);
            i.reason = i.reason.take().map(|r| scrub_home(&r, &text));
        }
        self
    }
}

/// `text` with each occurrence of the home folder `home` written as `~`. An occurrence counts only
/// when the next character cannot continue a folder name, so `/Users/me2` is not inside `/Users/me`.
fn scrub_home(text: &str, home: &str) -> String {
    if home.is_empty() { return text.to_string(); }
    let (mut out, mut rest) = (String::new(), text);
    while let Some(at) = rest.find(home) {
        let after = &rest[at + home.len()..];
        let ends_name = after.chars().next().is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '_' | '-')));
        out.push_str(&rest[..at]);
        out.push_str(if ends_name { "~" } else { home });
        rest = after;
    }
    out.push_str(rest);
    out
}

fn legacy_row(ids: &str, paths: &str, refused: &str) -> rusqlite::Result<Vec<TrashItemRow>> {
    let (ids, paths): (Vec<String>, Vec<String>) = (json_column(ids)?, json_column(paths)?);
    let moved = ids.into_iter().zip(paths).map(|(id, path)| TrashItemRow { id, path, label: "unknown".into(), state: "moved".into(), code: None, reason: None, bytes: None });
    let refused: Vec<serde_json::Value> = json_column(refused)?;
    let refused = refused.into_iter().map(|v| TrashItemRow { id: v["id"].as_str().unwrap_or("").into(), path: v["path"].as_str().unwrap_or("").into(), label: "unknown".into(), state: "refused".into(), code: None, reason: v["reason"].as_str().map(str::to_string), bytes: None });
    Ok(moved.chain(refused).collect())
}

fn sql_int(v: u64) -> rusqlite::Result<i64> { i64::try_from(v).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e))) }
fn rust_int(v: i64) -> rusqlite::Result<u64> { u64::try_from(v).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(e))) }
fn json_column<T: serde::de::DeserializeOwned>(text: &str) -> rusqlite::Result<T> { serde_json::from_str(text).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))) }

impl History {
    /// Writes one `pending` row per item of a request to move to the Trash, in one transaction, and
    /// returns the request's id. Called before anything is checked or moved: when it fails the caller
    /// moves nothing.
    pub fn begin_trash(&self, ts: u64, items: &[(&str, &str, &str)]) -> rusqlite::Result<u64> {
        let tx = self.0.unchecked_transaction()?;
        let request: i64 = tx.query_row("SELECT COALESCE(MAX(request), 0) + 1 FROM trash_items", [], |r| r.get(0))?;
        let ts = sql_int(ts)?;
        for (id, path, label) in items {
            tx.execute("INSERT INTO trash_items(request,ts,item_id,path,label,state) VALUES(?1,?2,?3,?4,?5,'pending')", params![request, ts, id, path, label])?;
        }
        // Whole requests only: the request holding the row just past the cap goes entirely, so no
        // request is ever left with some of its items missing.
        tx.execute("DELETE FROM trash_items WHERE request <= COALESCE((SELECT request FROM trash_items ORDER BY id DESC LIMIT 1 OFFSET ?1), 0)", [TRASH_KEEP])?;
        tx.execute("DELETE FROM trash_moves WHERE id <= (SELECT MAX(id) FROM trash_moves) - ?1", [TRASH_KEEP])?;
        tx.commit()?;
        rust_int(request)
    }

    /// Marks rows a stopped app left as `pending` or `moving` as `interrupted`: whether those items
    /// moved is unknown. Run once at startup, before any request, because a request in flight owns its
    /// `pending` and `moving` rows. Returns how many rows were changed.
    pub fn reconcile_interrupted_trash(&self) -> rusqlite::Result<usize> {
        self.0.execute("UPDATE trash_items SET state='interrupted', reason=?1 WHERE state IN ('pending','moving')", [INTERRUPTED_REASON])
    }

    /// Sets one item's state (`moving`, `moved`, `refused`, `failed` or `not_attempted`). An item the
    /// request never listed is an error, so a lost row cannot pass silently.
    pub fn mark_trash(&self, request: u64, item_id: &str, state: &str, code: Option<&str>, reason: Option<&str>, bytes: Option<u64>) -> rusqlite::Result<()> {
        let bytes = bytes.map(sql_int).transpose()?;
        let n = self.0.execute("UPDATE trash_items SET state=?3, code=?4, reason=?5, bytes=?6 WHERE request=?1 AND item_id=?2", params![sql_int(request)?, item_id, state, code, reason, bytes])?;
        if n == 1 { Ok(()) } else { Err(rusqlite::Error::StatementChangedRows(n)) }
    }

    /// The newest `limit` requests, newest first, then rows from the first audit table to fill the rest.
    pub fn recent_trash_moves(&self, limit: usize) -> rusqlite::Result<Vec<TrashMoveRow>> {
        let limit = sql_int(limit as u64)?;
        let mut st = self.0.prepare("SELECT request,ts,item_id,path,label,state,code,reason,bytes FROM trash_items WHERE request IN (SELECT DISTINCT request FROM trash_items ORDER BY request DESC LIMIT ?1) ORDER BY request DESC, id ASC")?;
        let mut rows: Vec<TrashMoveRow> = Vec::new();
        for r in st.query_map([limit], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, TrashItemRow { id: r.get(2)?, path: r.get(3)?, label: r.get(4)?, state: r.get(5)?, code: r.get(6)?, reason: r.get(7)?, bytes: r.get::<_, Option<i64>>(8)?.map(rust_int).transpose()? })))? {
            let (request, ts, item) = r?;
            let request = rust_int(request)?;
            match rows.last_mut() {
                Some(last) if last.request == Some(request) => last.items.push(item),
                _ => rows.push(TrashMoveRow { request: Some(request), ts: rust_int(ts)?, bytes: 0, items: vec![item] }),
            }
        }
        for row in &mut rows { row.bytes = row.items.iter().filter(|i| i.state == "moved").filter_map(|i| i.bytes).fold(0, u64::saturating_add); }
        let left = limit - i64::try_from(rows.len()).unwrap_or(limit);
        if left > 0 {
            let mut st = self.0.prepare("SELECT ts,ids,paths,bytes,refused FROM trash_moves ORDER BY id DESC LIMIT ?1")?;
            for r in st.query_map([left], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, String>(4)?)))? {
                let (ts, ids, paths, bytes, refused) = r?;
                match legacy_row(&ids, &paths, &refused) {
                    Ok(items) => rows.push(TrashMoveRow { request: None, ts: rust_int(ts)?, bytes: rust_int(bytes)?, items }),
                    Err(e) => {
                        // One damaged old row must not hide the rest of the history.
                        eprintln!("monitor-lizard: a row of the first Trash audit table could not be read ({e}); it is listed as unreadable");
                        let item = TrashItemRow { id: String::new(), path: String::new(), label: "unknown".into(), state: "unreadable".into(), code: None, reason: Some("this old record is damaged and could not be read".into()), bytes: None };
                        rows.push(TrashMoveRow { request: None, ts: rust_int(ts).unwrap_or(0), bytes: 0, items: vec![item] });
                    }
                }
            }
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trash_db(name: &str) -> (std::path::PathBuf, History) {
        let dir = std::env::temp_dir().join(format!("ml-trash-hist-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let h = History::at(&dir.join("h.db")).unwrap();
        (dir, h)
    }

    #[test]
    fn a_trash_request_is_written_as_pending_rows_and_updated_with_each_result() {
        let (dir, h) = trash_db("rows");
        let r = h.begin_trash(7, &[("a", "/Users/x/Library/Caches/a", "safe"), ("b", "/Users/x/b", "yours")]).unwrap();
        let rows = h.recent_trash_moves(10).unwrap();
        assert_eq!((rows.len(), rows[0].request, rows[0].ts), (1, Some(r), 7));
        assert!(rows[0].items.iter().all(|i| i.state == "pending"), "a crash here still leaves a record of both");
        h.mark_trash(r, "a", "moving", None, None, None).unwrap();
        h.mark_trash(r, "a", "moved", None, None, Some(42)).unwrap();
        h.mark_trash(r, "b", "refused", Some("changed"), Some("changed since the scan; rescan"), None).unwrap();
        let rows = h.recent_trash_moves(10).unwrap();
        assert_eq!(rows[0].bytes, 42);
        assert_eq!((rows[0].items[0].state.as_str(), rows[0].items[0].bytes), ("moved", Some(42)));
        assert_eq!((rows[0].items[1].state.as_str(), rows[0].items[1].code.as_deref()), ("refused", Some("changed")));
        assert!(h.mark_trash(r, "nope", "moved", None, None, None).is_err(), "an unlisted item is an error, not a silent no-op");
        assert_eq!(h.begin_trash(8, &[("c", "/p", "safe")]).unwrap(), r + 1);
        assert_eq!(h.recent_trash_moves(1).unwrap()[0].request, Some(r + 1), "newest first");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_trash_audit_keeps_only_the_newest_rows() {
        let (dir, h) = trash_db("keep");
        // The first TRASH_KEEP requests go in as one statement. Committing each one separately costs a disk sync apiece, which
        // took minutes on a hosted runner; only the last five go through begin_trash, so the pruning is still what is tested.
        h.0.execute("INSERT INTO trash_items(request,ts,item_id,path,label,state) WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < ?1) SELECT x, x - 1, 'a', '/p', 'safe', 'pending' FROM n", [TRASH_KEEP]).unwrap();
        for n in TRASH_KEEP as u64..(TRASH_KEEP as u64 + 5) { h.begin_trash(n, &[("a", "/p", "safe")]).unwrap(); }
        let rows = h.recent_trash_moves(100_000).unwrap();
        assert_eq!(rows.len(), TRASH_KEEP as usize);
        assert_eq!(rows[0].ts, TRASH_KEEP as u64 + 4, "the newest survived");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_row_from_the_first_audit_table_is_still_listed() {
        let (dir, h) = trash_db("legacy");
        h.0.execute("INSERT INTO trash_moves(ts,ids,paths,bytes,refused) VALUES(3,'[\"a\"]','[\"/Users/x/a\"]',9,'[{\"id\":\"b\",\"path\":\"/Users/x/b\",\"reason\":\"changed since the scan; rescan\"}]')", []).unwrap();
        let rows = h.recent_trash_moves(10).unwrap();
        assert_eq!((rows[0].request, rows[0].bytes, rows[0].items.len()), (None, 9, 2));
        assert_eq!((rows[0].items[0].state.as_str(), rows[0].items[1].state.as_str()), ("moved", "refused"));
        h.0.execute("UPDATE trash_moves SET ids='not json'", []).unwrap();
        h.0.execute("INSERT INTO trash_moves(ts,ids,paths,bytes,refused) VALUES(4,'[]','[]',1,'[]')", []).unwrap();
        let rows = h.recent_trash_moves(10).unwrap();
        assert_eq!(rows.len(), 2, "one damaged row does not hide the others");
        assert_eq!((rows[1].items[0].state.as_str(), rows[1].request), ("unreadable", None));
        assert_eq!(rows[0].ts, 4, "the good row is still listed");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn big_numbers_are_refused_not_wrapped_and_history_shows_home_as_a_tilde() {
        assert!(sql_int(u64::MAX).is_err() && sql_int(5).is_ok() && rust_int(-1).is_err());
        let row = TrashMoveRow { request: Some(1), ts: 1, bytes: 0, items: ["/Users/x/Library/a", "/Applications/B.app", "/Users/x", "/Users/xavier/c"].iter()
            .map(|p| TrashItemRow { id: "i".into(), path: p.to_string(), label: "safe".into(), state: "moved".into(), code: None, reason: None, bytes: None }).collect() }
            .with_home_as_tilde(std::path::Path::new("/Users/x"));
        assert_eq!(row.items.iter().map(|i| i.path.as_str()).collect::<Vec<_>>(), ["~/Library/a", "/Applications/B.app", "~", "/Users/xavier/c"]);
    }

    #[test]
    fn the_cap_removes_whole_requests_never_part_of_one() {
        let (dir, h) = trash_db("whole");
        let big: Vec<(&str, &str, &str)> = (0..50).map(|_| ("i", "/p", "safe")).collect();
        for n in 0..(TRASH_KEEP as u64 / 50 + 3) { h.begin_trash(n, &big).unwrap(); }
        let rows = h.recent_trash_moves(100_000).unwrap();
        assert!(rows.iter().all(|r| r.items.len() == 50), "every surviving request is whole");
        assert!(rows.len() as i64 * 50 <= TRASH_KEEP && rows.len() as i64 * 50 > TRASH_KEEP - 50);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rows_left_pending_or_moving_become_interrupted_at_startup_and_finished_ones_do_not() {
        let (dir, h) = trash_db("interrupted");
        let r = h.begin_trash(1, &[("a", "/p/a", "safe"), ("b", "/p/b", "safe"), ("c", "/p/c", "safe")]).unwrap();
        h.mark_trash(r, "a", "moved", None, None, Some(5)).unwrap();
        h.mark_trash(r, "b", "moving", None, None, None).unwrap();
        assert_eq!(h.reconcile_interrupted_trash().unwrap(), 2);
        let items = &h.recent_trash_moves(5).unwrap()[0].items;
        assert_eq!(items.iter().map(|i| i.state.as_str()).collect::<Vec<_>>(), ["moved", "interrupted", "interrupted"]);
        assert!(items[1].reason.as_deref().unwrap().contains("may or may not have moved"));
        assert_eq!(h.reconcile_interrupted_trash().unwrap(), 0, "a second pass changes nothing");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn home_is_scrubbed_from_paths_and_reason_text_but_only_as_a_whole_folder_name() {
        let mk = |path: &str, reason: &str| TrashMoveRow { request: Some(1), ts: 1, bytes: 0, items: vec![TrashItemRow { id: "i".into(), path: path.into(), label: "safe".into(), state: "failed".into(), code: None, reason: Some(reason.into()), bytes: None }] };
        let row = mk("/Users/me2/x", "cannot move /Users/me/Library/Caches/a: busy; also /Users/me2/b and /Users/me.").with_home_as_tilde(std::path::Path::new("/Users/me/"));
        assert_eq!(row.items[0].path, "/Users/me2/x", "another user's folder is not this home");
        assert_eq!(row.items[0].reason.as_deref(), Some("cannot move ~/Library/Caches/a: busy; also /Users/me2/b and ~."));
    }
}
