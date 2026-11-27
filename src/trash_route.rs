//! `POST /api/trash`, in three steps kept apart so each can be tested alone: `parse` (what the query
//! says, strictly), policy (is this request allowed to move anything: a trusted home, ids from the
//! latest scan, a confirmation token for exactly this selection) and I/O (the audit rows and the moves).
//!
//! Contract, in one place:
//! - Without `dry=0` a request is a dry run. Only `dry=1` or no `dry` is a preview and only `dry=0`
//!   moves; any other value is a 400, so a typo never moves a file.
//! - A preview that would move something returns `confirm_token`. A real move must send it back with
//!   the same ids; it is single use, short lived, kept in memory only, and never logged or stored.
//! - Items labelled yours also need `confirm_yours=1`.
//! - Every real request writes an audit row per item before anything moves; if that fails nothing moves.
use crate::{actions, history::{self, History}, storage::{self, trash::{self, Audit, Code, Counts, Entry, ItemResult, Mover, NoAudit, Policy, Refusal}, Item}};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a preview's confirmation stays valid.
pub const TOKEN_TTL: Duration = Duration::from_secs(600);
/// Outstanding confirmations kept; the oldest is dropped first, so a flood of previews cannot grow memory.
const MAX_TOKENS: usize = 32;

/// Said in every reply: the move does not go through Finder, so "Put Back" may be missing.
pub const NOTE: &str = "Items go to your Trash, not to a permanent delete. Finder's Put Back may not be offered for them; the original path of every item is recorded in History.";

/// A refusal of the whole request, with a stable `code` for the page.
#[derive(Debug, PartialEq)]
pub struct ApiError { pub status: u16, pub code: &'static str, pub message: String }

fn fail(status: u16, code: &'static str, message: impl Into<String>) -> ApiError { ApiError { status, code, message: message.into() } }

#[derive(Debug, PartialEq)]
pub enum Mode { Preview, Commit { token: Option<String> } }

#[derive(Debug, PartialEq)]
pub struct Request { pub ids: Vec<String>, pub confirm_yours: bool, pub mode: Mode }

/// The one value of `name`, `None` when absent. A repeated name is refused: with two values it would be
/// a guess which one counts.
fn one(query: &str, name: &str) -> Result<Option<String>, ApiError> {
    let mut found = query.split('&').filter_map(|kv| kv.split_once('=').or(Some((kv, ""))).filter(|(k, _)| *k == name).map(|(_, v)| crate::server::urldecode(v)));
    let first = found.next();
    if found.next().is_some() { return Err(fail(400, "repeated_parameter", format!("{name} was given more than once"))); }
    Ok(first)
}

/// Absent or `0` is off, `1` is on, anything else is an error.
fn flag(query: &str, name: &str) -> Result<bool, ApiError> {
    match one(query, name)?.as_deref() {
        None | Some("0") => Ok(false),
        Some("1") => Ok(true),
        Some(_) => Err(fail(400, "bad_flag", format!("{name} must be 0 or 1"))),
    }
}

pub fn parse(query: &str) -> Result<Request, ApiError> {
    let ids = trash::parse_ids(&one(query, "ids")?.unwrap_or_default()).map_err(|e| fail(400, "bad_ids", e))?;
    let confirm_yours = flag(query, "confirm_yours")?;
    // Only an explicit `dry=0` moves anything: a missing or misspelt value must not.
    let mode = match one(query, "dry")?.as_deref() {
        None | Some("1") => Mode::Preview,
        Some("0") => Mode::Commit { token: one(query, "confirm_token")? },
        Some(_) => return Err(fail(400, "bad_dry", "dry must be 1 (preview) or 0 (move); leave it out for a preview")),
    };
    Ok(Request { ids, confirm_yours, mode })
}

/// Why a confirmation was not accepted.
#[derive(Debug, PartialEq)]
pub enum TokenError { Missing, Unknown, Expired, OtherSelection }

impl TokenError {
    fn into_api(self) -> ApiError {
        match self {
            TokenError::Missing => fail(400, "confirmation_required", "a real move needs the confirm_token from a preview of the same ids"),
            TokenError::Unknown => fail(409, "confirmation_invalid", "that confirmation was not issued, or was already used; preview again"),
            TokenError::Expired => fail(409, "confirmation_expired", "that confirmation expired; preview again"),
            TokenError::OtherSelection => fail(409, "confirmation_mismatch", "that confirmation was issued for different items or an older scan; preview again"),
        }
    }
}

struct Issued { token: String, binding: String, expires: Instant }

/// Confirmations handed out by previews. In memory only, so a restart forgets them.
#[derive(Default)]
pub struct ConfirmTokens(Mutex<Vec<Issued>>);

/// What a confirmation is bound to: the scan, and each requested id with its label.
pub fn binding(generation: u64, items: &[Item]) -> String {
    let mut parts: Vec<String> = items.iter().map(|i| format!("{}:{}", i.id, i.label.as_str())).collect();
    parts.sort();
    format!("{generation}|{}", parts.join(","))
}

impl ConfirmTokens {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Issued>> { self.0.lock().unwrap_or_else(|e| e.into_inner()) }

    pub fn issue(&self, binding: String, now: Instant) -> Result<String, String> {
        let token = crate::server::random_token().map_err(|e| format!("no random source for a confirmation: {e}"))?;
        let mut v = self.lock();
        v.retain(|t| t.expires > now);
        if v.len() >= MAX_TOKENS { v.remove(0); }
        v.push(Issued { token: token.clone(), binding, expires: now + TOKEN_TTL });
        Ok(token)
    }

    /// Accepts a token once. It is used up by any attempt, matching or not.
    pub fn redeem(&self, token: Option<&str>, binding: &str, now: Instant) -> Result<(), TokenError> {
        let token = token.filter(|t| !t.is_empty()).ok_or(TokenError::Missing)?;
        let mut v = self.lock();
        let at = v.iter().position(|t| t.token == token).ok_or(TokenError::Unknown)?;
        let t = v.remove(at);
        if t.expires <= now { Err(TokenError::Expired) } else if t.binding != binding { Err(TokenError::OtherSelection) } else { Ok(()) }
    }
}

/// One removal at a time: two requests moving the same items at once would race each other's checks.
static BUSY: AtomicBool = AtomicBool::new(false);
pub struct Busy;
impl Busy {
    pub fn try_begin() -> Option<Busy> { (!BUSY.swap(true, Ordering::SeqCst)).then_some(Busy) }
}
impl Drop for Busy { fn drop(&mut self) { BUSY.store(false, Ordering::SeqCst); } }

pub fn busy_reply() -> (u16, String) { error_reply(&fail(409, "busy", "another removal is still running; try again in a moment")) }

/// What the reply to a request that ran says. Additive over the first shape: `moved` and `refused` are kept.
#[derive(Serialize)]
struct Reply<'a> {
    /// True when every requested item moved (or would) and the audit log was written.
    ok: bool,
    dry_run: bool,
    /// The audit entry of this request; null for a preview.
    history_id: Option<u64>,
    counts: Counts,
    bytes: u64,
    items: &'a [ItemResult],
    moved: Vec<Entry>,
    refused: Vec<Refusal>,
    /// The item whose move failed, if one did.
    failed: Option<&'a ItemResult>,
    confirm_token: Option<String>,
    confirm_token_expires_in_secs: Option<u64>,
    audit_error: Option<String>,
    note: &'static str,
}

fn error_reply(e: &ApiError) -> (u16, String) {
    match serde_json::to_string(&serde_json::json!({ "ok": false, "error": e.message, "code": e.code })) {
        Ok(body) => (e.status, body),
        Err(_) => (500, r#"{"ok":false,"error":"could not build the error reply","code":"reply_failed"}"#.to_string()),
    }
}

fn render(o: &trash::Outcome, history_id: Option<u64>, token: Option<String>) -> (u16, String) {
    let reply = Reply { ok: o.ok(), dry_run: o.dry_run, history_id, counts: o.counts(), bytes: o.bytes, items: &o.items, moved: o.moved(), refused: o.refused(),
        failed: o.items.iter().find(|i| i.status == trash::Status::Failed), confirm_token_expires_in_secs: token.as_ref().map(|_| TOKEN_TTL.as_secs()),
        confirm_token: token, audit_error: o.audit_error.clone(), note: NOTE };
    // A run whose audit log failed part way is an error for the caller, but its body still lists what moved.
    let status = if o.audit_error.is_some() { 500 } else { 200 };
    match serde_json::to_string(&reply) {
        Ok(body) => (status, body),
        Err(e) => error_reply(&fail(500, "reply_failed", format!("the request ran but its reply could not be built ({e}); see History for what moved"))),
    }
}

/// Writes the audit rows of one real request.
struct DbAudit<'a> { h: &'a History, request: u64 }

impl Audit for DbAudit<'_> {
    fn attempting(&mut self, item: &Item) -> Result<(), String> { self.h.mark_trash(self.request, &item.id, "moving", None, None, None).map_err(|e| e.to_string()) }
    fn finished(&mut self, r: &ItemResult) -> Result<(), String> {
        self.h.mark_trash(self.request, &r.id, r.status.as_str(), r.code.map(Code::as_str), r.reason.as_deref(), r.bytes).map_err(|e| e.to_string())
    }
}

/// Everything the route reads from the outside, so a test can give it a scratch home, database and mover.
pub struct Ctx<'a> {
    pub cache: &'a storage::Cache,
    pub tokens: &'a ConfirmTokens,
    pub db: &'a Path,
    pub home: Result<PathBuf, &'static str>,
    pub apps: &'a Path,
    pub uid: u32,
    pub mover: Mover<'a>,
    pub now: Instant,
    pub unix_now: u64,
    pub budget: Duration,
}

pub fn reply(query: &str, ctx: &Ctx) -> (u16, String) {
    match handle(query, ctx) { Ok(r) => r, Err(e) => error_reply(&e) }
}

fn handle(query: &str, ctx: &Ctx) -> Result<(u16, String), ApiError> {
    let req = parse(query)?;
    let home = ctx.home.clone().map_err(|why| fail(400, "home_untrusted", why))?;
    let (generation, items) = ctx.cache.find_in_generation(&req.ids).map_err(|e| fail(400, "unknown_id", e))?;
    let pol = Policy { home: &home, apps: ctx.apps, uid: ctx.uid, confirm_yours: req.confirm_yours, deadline: ctx.now + ctx.budget };
    let bound = binding(generation, &items);
    match req.mode {
        Mode::Preview => {
            let o = trash::run(&items, &pol, true, ctx.mover, &mut NoAudit);
            let token = if o.counts().would_move > 0 { Some(ctx.tokens.issue(bound, ctx.now).map_err(|e| fail(500, "no_confirmation", e))?) } else { None };
            Ok(render(&o, None, token))
        }
        Mode::Commit { token } => {
            // The audit comes first: if it cannot be opened or written, nothing has moved.
            let h = History::at(ctx.db).map_err(|e| fail(500, "audit_unavailable", format!("the audit log is unavailable, so nothing was moved: {}", history::unavailable(&e))))?;
            ctx.tokens.redeem(token.as_deref(), &bound, ctx.now).map_err(TokenError::into_api)?;
            let rows: Vec<(&str, &str, &str)> = items.iter().map(|i| (i.id.as_str(), i.path.as_str(), i.label.as_str())).collect();
            let request = h.begin_trash(ctx.unix_now, &rows).map_err(|e| fail(500, "audit_unavailable", format!("the audit log could not be written, so nothing was moved: {}", history::unavailable(&e))))?;
            let o = trash::run(&items, &pol, false, ctx.mover, &mut DbAudit { h: &h, request });
            Ok(render(&o, Some(request), None))
        }
    }
}

/// The route as the server runs it: the real home, history database, `/Applications` and Trash.
pub fn serve_reply(query: &str, cache: &storage::Cache, tokens: &ConfirmTokens) -> (u16, String) {
    let unix_now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    reply(query, &Ctx { cache, tokens, db: &history::db_path(), home: storage::home_or_reason(), apps: Path::new("/Applications"), uid: storage::current_uid(), mover: &actions::trash, now: Instant::now(), unix_now, budget: trash::REQUEST_BUDGET })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{item_id, trash::tests::{cache_dir, scanned, temp}, Cache, Label, Storage};
    use std::cell::RefCell;
    use std::fs;

    struct Fx { home: PathBuf, apps: PathBuf, db: PathBuf, cache: Cache, tokens: ConfirmTokens, items: Vec<Item> }

    fn fx(name: &str, labels: &[Label]) -> Fx {
        let (home, apps, dir) = (temp(&format!("rt-{name}")), temp(&format!("rt-{name}-apps")), temp(&format!("rt-{name}-db")));
        let items: Vec<Item> = labels.iter().enumerate().map(|(i, l)| scanned(&cache_dir(&home, &format!("d{i}")), *l)).collect();
        let mut s = Storage::fake(1);
        s.items.extend(items.iter().cloned());
        Fx { home, apps, db: dir.join("h.db"), cache: Cache::with_result(s), tokens: ConfirmTokens::default(), items }
    }

    impl Fx {
        fn ids(&self, n: usize) -> String { self.items[..n].iter().map(|i| i.id.as_str()).collect::<Vec<_>>().join(",") }
        fn call(&self, query: &str, mover: Mover) -> (u16, serde_json::Value) {
            let ctx = Ctx { cache: &self.cache, tokens: &self.tokens, db: &self.db, home: Ok(self.home.clone()), apps: &self.apps, uid: crate::storage::current_uid(), mover, now: Instant::now(), unix_now: 100, budget: Duration::from_secs(60) };
            let (code, body) = reply(query, &ctx);
            (code, serde_json::from_str(&body).expect("the reply is JSON"))
        }
        fn preview(&self, n: usize) -> String { self.preview_with(n, "") }
        fn preview_with(&self, n: usize, extra: &str) -> String {
            let (code, v) = self.call(&format!("ids={}&dry=1{extra}", self.ids(n)), &never);
            assert_eq!(code, 200, "{v}");
            v["confirm_token"].as_str().expect("a preview that would move something returns a token").to_string()
        }
        fn audit_rows(&self) -> Vec<crate::history::TrashMoveRow> { History::at(&self.db).unwrap().recent_trash_moves(100).unwrap() }
        fn exists(&self, i: usize) -> bool { Path::new(&self.items[i].path).exists() }
    }

    fn never(_: &Path) -> Result<(), trash::MoveError> { panic!("must not move") }
    fn remove(p: &Path) -> Result<(), trash::MoveError> { fs::remove_dir_all(p).map_err(|e| trash::MoveError::Other(e.to_string())) }

    #[test]
    fn only_dry_0_moves_and_every_other_spelling_is_a_preview_or_a_400() {
        let ids = "0123456789abcdef";
        for q in [format!("ids={ids}"), format!("ids={ids}&dry=1")] { assert_eq!(parse(&q).unwrap().mode, Mode::Preview, "{q}"); }
        assert_eq!(parse(&format!("ids={ids}&dry=0&confirm_token=t")).unwrap().mode, Mode::Commit { token: Some("t".into()) });
        for bad in ["true", "false", "yes", "no", "", "01", "00", "10", "2", "-1", "DRY", "Dry", "0%20", "1%200", "0,1", "off", "on", "tru", "ture", "one"] {
            let e = parse(&format!("ids={ids}&dry={bad}")).unwrap_err();
            assert_eq!((e.status, e.code), (400, "bad_dry"), "dry={bad:?} must not be understood");
        }
        assert_eq!(parse(&format!("ids={ids}&dry=1&dry=0")).unwrap_err().code, "repeated_parameter");
        assert_eq!(parse(&format!("ids={ids}&dry")).unwrap_err().code, "bad_dry", "a bare key is not a value");
        assert_eq!(parse(&format!("ids={ids}&dry=1&confirm_yours=true")).unwrap_err().code, "bad_flag");
        assert!(parse(&format!("ids={ids}&confirm_yours=1")).unwrap().confirm_yours);
        assert_eq!(parse("dry=0").unwrap_err().code, "bad_ids");
    }

    #[test]
    fn a_misspelt_dry_value_over_the_whole_route_moves_nothing() {
        let f = fx("typo", &[Label::Safe]);
        for bad in ["true", "yes", "", "ture", "O"] {
            let (code, v) = f.call(&format!("ids={}&dry={bad}&confirm_token=x", f.ids(1)), &never);
            assert_eq!((code, v["code"].as_str()), (400, Some("bad_dry")), "{bad}");
        }
        assert!(f.exists(0) && f.audit_rows().is_empty());
    }

    #[test]
    fn a_preview_touches_nothing_writes_no_audit_row_and_returns_a_token() {
        let f = fx("preview", &[Label::Safe, Label::Review]);
        let (code, v) = f.call(&format!("ids={}", f.ids(2)), &never);
        assert_eq!(code, 200);
        assert_eq!((v["ok"].clone(), v["dry_run"].clone(), v["counts"]["would_move"].clone(), v["history_id"].clone()), (true.into(), true.into(), 2.into(), serde_json::Value::Null));
        assert!(v["confirm_token"].as_str().unwrap().len() >= 32 && v["note"].as_str().unwrap().contains("Put Back"));
        assert!(f.exists(0) && f.exists(1));
        assert!(!f.db.exists() || f.audit_rows().is_empty(), "a dry run writes no audit row");
    }

    #[test]
    fn a_real_move_needs_the_token_from_a_preview_of_the_same_ids() {
        let f = fx("token", &[Label::Safe, Label::Safe]);
        let q = |extra: &str| format!("ids={}&dry=0{extra}", f.ids(1));
        assert_eq!(f.call(&q(""), &never).1["code"], "confirmation_required");
        assert_eq!(f.call(&q("&confirm_token=nonsense"), &never).1["code"], "confirmation_invalid");
        // A token for one id does not move two.
        let t = f.preview(1);
        let (code, v) = f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(2)), &never);
        assert_eq!((code, v["code"].as_str()), (409, Some("confirmation_mismatch")));
        assert!(f.exists(0) && f.exists(1) && f.audit_rows().is_empty());
        // The mismatching attempt used the token up.
        assert_eq!(f.call(&q(&format!("&confirm_token={t}")), &never).1["code"], "confirmation_invalid");
        // The right one works once.
        let t = f.preview(1);
        let (code, v) = f.call(&q(&format!("&confirm_token={t}")), &remove);
        assert_eq!((code, v["ok"].clone(), v["counts"]["moved"].clone()), (200, true.into(), 1.into()));
        assert!(!f.exists(0) && f.exists(1));
        assert_eq!(f.call(&q(&format!("&confirm_token={t}")), &never).1["code"], "confirmation_invalid", "single use");
    }

    #[test]
    fn a_token_is_bound_to_the_labels_the_scan_and_expires() {
        let f = fx("bound", &[Label::Safe]);
        let t = f.preview(1);
        // A new scan finished since the preview.
        let mut s = Storage::fake(2);
        s.items.extend(f.items.iter().cloned());
        f.cache.replace_result(s);
        assert_eq!(f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(1)), &never).1["code"], "confirmation_mismatch");
        // The same scan with a changed label.
        let t = f.preview(1);
        let mut relabelled = f.items.clone();
        relabelled[0].label = Label::Yours;
        let mut s = Storage::fake(3);
        s.items.extend(relabelled);
        f.cache.replace_result(s);
        assert_eq!(f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(1)), &never).1["code"], "confirmation_mismatch");
        // Expiry, on the token store itself.
        let tokens = ConfirmTokens::default();
        let now = Instant::now();
        let t = tokens.issue("b".into(), now).unwrap();
        assert_eq!(tokens.redeem(Some(&t), "b", now + TOKEN_TTL), Err(TokenError::Expired));
        let t = tokens.issue("b".into(), now).unwrap();
        assert_eq!(tokens.redeem(Some(&t), "b", now + TOKEN_TTL - Duration::from_secs(1)), Ok(()));
        for _ in 0..(MAX_TOKENS + 10) { tokens.issue("b".into(), now).unwrap(); }
        assert!(tokens.lock().len() <= MAX_TOKENS, "outstanding confirmations are capped");
    }

    #[test]
    fn a_yours_item_needs_confirm_yours_together_with_the_token() {
        let f = fx("yours", &[Label::Yours]);
        let (_, v) = f.call(&format!("ids={}&dry=1", f.ids(1)), &never);
        assert!(v["confirm_token"].is_null(), "nothing would move, so no token");
        let t = f.preview_with(1, "&confirm_yours=1");
        let (_, v) = f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(1)), &never);
        assert_eq!((v["ok"].clone(), v["items"][0]["code"].clone()), (false.into(), "needs_yours_confirmation".into()));
        let t = f.preview_with(1, "&confirm_yours=1");
        let (_, v) = f.call(&format!("ids={}&dry=0&confirm_yours=1&confirm_token={t}", f.ids(1)), &remove);
        assert_eq!(v["counts"]["moved"], 1);
        // `confirm_yours=1` alone, with no token, never moves.
        let g = fx("yours2", &[Label::Yours]);
        let (code, v) = g.call(&format!("ids={}&dry=0&confirm_yours=1", g.ids(1)), &never);
        assert_eq!((code, v["code"].as_str()), (400, Some("confirmation_required")));
    }

    #[test]
    fn the_audit_rows_exist_before_each_move_and_say_moving() {
        let f = fx("order", &[Label::Safe, Label::Safe]);
        let t = f.preview(2);
        let seen: RefCell<Vec<Vec<String>>> = RefCell::new(vec![]);
        let mover = |p: &Path| {
            let rows = f.audit_rows();
            assert_eq!(rows.len(), 1, "one request");
            seen.borrow_mut().push(rows[0].items.iter().map(|i| format!("{}={}", Path::new(&i.path).file_name().unwrap().to_string_lossy(), i.state)).collect());
            remove(p)
        };
        let (_, v) = f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(2)), &mover);
        assert_eq!(*seen.borrow(), [vec!["d0=moving", "d1=pending"], vec!["d0=moved", "d1=moving"]]);
        let rows = f.audit_rows();
        assert_eq!((rows.len(), rows[0].request, v["history_id"].as_u64()), (1, Some(1), Some(1)));
        assert!(rows[0].items.iter().all(|i| i.state == "moved" && i.bytes.is_some_and(|b| b > 0)));
    }

    #[test]
    fn an_audit_log_that_cannot_be_opened_moves_nothing() {
        let mut f = fx("nodb", &[Label::Safe]);
        let t = f.preview(1);
        // A folder where the database should be.
        f.db = temp("rt-nodb-folder");
        let (code, v) = f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(1)), &never);
        assert_eq!((code, v["code"].as_str(), v["ok"].clone()), (500, Some("audit_unavailable"), false.into()));
        assert!(f.exists(0));
    }

    fn lock_table(db: &Path, event: &str) {
        let c = rusqlite::Connection::open(db).unwrap();
        c.execute_batch(&format!("CREATE TRIGGER lock_{event} BEFORE {event} ON trash_items BEGIN SELECT RAISE(ABORT, 'database is locked'); END;")).unwrap();
    }

    #[test]
    fn an_audit_row_that_cannot_be_inserted_moves_nothing_and_is_a_500() {
        let f = fx("noinsert", &[Label::Safe, Label::Safe]);
        drop(History::at(&f.db).unwrap());
        lock_table(&f.db, "INSERT");
        let t = f.preview(2);
        let (code, v) = f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(2)), &never);
        assert_eq!((code, v["code"].as_str()), (500, Some("audit_unavailable")));
        assert!(!v.to_string().contains(&t), "the confirmation is never echoed");
        assert!(f.exists(0) && f.exists(1));
    }

    #[test]
    fn an_audit_row_that_cannot_be_updated_before_a_move_stops_that_item_and_the_rest() {
        let f = fx("noupdate", &[Label::Safe, Label::Safe]);
        drop(History::at(&f.db).unwrap());
        lock_table(&f.db, "UPDATE");
        let t = f.preview(2);
        let (code, v) = f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(2)), &never);
        assert_eq!(code, 500);
        assert_eq!((v["ok"].clone(), v["items"][0]["status"].clone(), v["items"][0]["code"].clone(), v["items"][1]["status"].clone()), (false.into(), "not_attempted".into(), "audit_failed".into(), "not_attempted".into()));
        assert!(v["audit_error"].is_string());
        assert!(f.exists(0) && f.exists(1));
        let rows = f.audit_rows();
        assert!(rows[0].items.iter().all(|i| i.state == "pending"), "the attempt is still on record");
    }

    #[test]
    fn a_crash_mid_batch_leaves_a_row_for_every_item() {
        let f = fx("crash", &[Label::Safe, Label::Safe, Label::Safe]);
        let t = f.preview(3);
        let n = RefCell::new(0);
        let mover = |p: &Path| { *n.borrow_mut() += 1; if *n.borrow() == 2 { panic!("simulated crash") } remove(p) };
        let q = format!("ids={}&dry=0&confirm_token={t}", f.ids(3));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.call(&q, &mover)));
        assert!(r.is_err());
        let states: Vec<String> = f.audit_rows()[0].items.iter().map(|i| i.state.clone()).collect();
        assert_eq!(states, ["moved", "moving", "pending"], "every requested item has a row, and the one in flight says moving");
    }

    #[test]
    fn a_failed_move_is_marked_failed_and_counted() {
        let f = fx("failmove", &[Label::Safe, Label::Safe, Label::Safe]);
        let t = f.preview(3);
        let n = RefCell::new(0);
        let mover = |p: &Path| { *n.borrow_mut() += 1; if *n.borrow() == 2 { Err(trash::MoveError::NotPermitted("Operation not permitted".into())) } else { remove(p) } };
        let (code, v) = f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(3)), &mover);
        assert_eq!(code, 200);
        assert_eq!((v["ok"].clone(), v["counts"].clone()), (false.into(), serde_json::json!({"would_move":0,"moved":1,"refused":0,"failed":1,"not_attempted":1})));
        assert_eq!((v["failed"]["id"].as_str(), v["failed"]["code"].as_str()), (Some(f.items[1].id.as_str()), Some("move_not_permitted")));
        assert_eq!(v["refused"].as_array().unwrap().len(), 2, "the old `refused` list still names the failed and the unattempted item");
        let states: Vec<String> = f.audit_rows()[0].items.iter().map(|i| i.state.clone()).collect();
        assert_eq!(states, ["moved", "failed", "not_attempted"]);
    }

    #[test]
    fn all_items_refused_is_a_200_with_ok_false_and_a_code_on_each() {
        let f = fx("refused", &[Label::Safe]);
        let t = f.preview(1);
        fs::write(Path::new(&f.items[0].path).join("blob"), vec![9u8; 500_000]).unwrap();
        let (code, v) = f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(1)), &never);
        assert_eq!((code, v["ok"].clone(), v["counts"]["refused"].clone(), v["refused"][0]["code"].clone()), (200, false.into(), 1.into(), "changed".into()));
        assert_eq!(v["items"][0]["status"], "refused");
        assert!(f.exists(0));
        assert_eq!(f.audit_rows()[0].items[0].state, "refused", "refusals are audited too");
    }

    #[test]
    fn an_exhausted_budget_reports_not_attempted_and_moves_nothing() {
        let f = fx("budget", &[Label::Safe, Label::Safe]);
        let t = f.preview(2);
        let ctx = Ctx { cache: &f.cache, tokens: &f.tokens, db: &f.db, home: Ok(f.home.clone()), apps: &f.apps, uid: crate::storage::current_uid(), mover: &never, now: Instant::now(), unix_now: 1, budget: Duration::ZERO };
        let (code, body) = reply(&format!("ids={}&dry=0&confirm_token={t}", f.ids(2)), &ctx);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!((code, v["counts"]["not_attempted"].clone(), v["items"][0]["code"].clone()), (200, 2.into(), "time_budget".into()));
        assert!(f.exists(0) && f.exists(1));
    }

    #[test]
    fn an_untrusted_home_refuses_the_request_before_any_confirmation_is_used() {
        let f = fx("home", &[Label::Safe]);
        let t = f.preview(1);
        let ctx = Ctx { cache: &f.cache, tokens: &f.tokens, db: &f.db, home: Err(crate::storage::reason::HOME_UNTRUSTED), apps: &f.apps, uid: crate::storage::current_uid(), mover: &never, now: Instant::now(), unix_now: 1, budget: Duration::from_secs(5) };
        let (code, body) = reply(&format!("ids={}&dry=0&confirm_token={t}", f.ids(1)), &ctx);
        assert_eq!((code, serde_json::from_str::<serde_json::Value>(&body).unwrap()["code"].clone()), (400, "home_untrusted".into()));
        let (code, _) = f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(1)), &remove);
        assert_eq!(code, 200, "the token was not burnt by the refusal");
    }

    #[test]
    fn an_id_the_scan_did_not_list_is_a_400() {
        let f = fx("unknown", &[Label::Safe]);
        let (code, v) = f.call(&format!("ids={}&dry=1", item_id("/etc/hosts")), &never);
        assert_eq!((code, v["code"].as_str()), (400, Some("unknown_id")));
    }

    #[test]
    fn the_confirmation_is_not_in_the_audit_log() {
        let f = fx("notoken", &[Label::Safe]);
        let t = f.preview(1);
        f.call(&format!("ids={}&dry=0&confirm_token={t}", f.ids(1)), &remove);
        let raw = fs::read(&f.db).unwrap();
        let wal = fs::read(f.db.with_extension("db-wal")).unwrap_or_default();
        assert!(![raw, wal].iter().any(|b| b.windows(t.len()).any(|w| w == t.as_bytes())));
    }

    #[test]
    fn only_one_removal_runs_at_a_time() {
        let a = Busy::try_begin().expect("free");
        assert!(Busy::try_begin().is_none());
        assert_eq!(busy_reply().0, 409);
        drop(a);
        assert!(Busy::try_begin().is_some());
    }
}
