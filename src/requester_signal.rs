//! `POST /api/requester/signal` and `GET /api/requester/status`: freeze (SIGSTOP), resume (SIGCONT), stop (SIGTERM)
//! or force stop (SIGKILL) the processes of a sudo requester chain (P1's `Hop`s), one process or a whole chain.
//!
//! Contract, in one place:
//! - A request names an `action` (`freeze`, `resume`, `stop`, `force_stop`), a `scope` (`one` or `chain`) and
//!   one `item=<pid>:<started>:<exe>` per process, child first (the order a chain is listed in). `started` and
//!   `exe` are what the collector captured for that hop. Up to `MAX_ITEMS` items.
//! - Without `dry=0` a request is a preview and changes nothing. Only `dry=1` or no `dry` is a preview and only
//!   `dry=0` signals; any other value is a 400.
//! - A preview that would signal something returns `confirm_token`: single use, ten minutes, in memory, tied to the
//!   action, the scope and the exact items. A real request must send it back.
//! - Every pid is checked fresh when previewing, again before the audit row, and once more immediately before
//!   its own signal. A pid is signalled only when it still has the captured start time and executable, belongs to
//!   you, and is not launchd, Monitor Lizard, or a parent or child of Monitor Lizard.
//! - The audit row (the `actions` table) is written before the first signal; if it cannot be written nothing is
//!   signalled. A signal that fails or is refused at the last moment gets a second row.
//! - Freeze and resume go parent first; stop and force stop go child first.
use crate::history::{self, History};
use crate::trash_route::{ConfirmTokens, IssueError, TokenError, TOKEN_TTL};
use crate::watcher::MAX_HOPS;
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

/// Processes one request may name. A chain holds at most this many hops.
pub const MAX_ITEMS: usize = MAX_HOPS;
/// Children listed per process; the count is always exact.
const CHILD_LIMIT: usize = 12;
const COMMAND_MAX: usize = crate::watcher::COMMAND_MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { Freeze, Resume, Stop, ForceStop }

impl Action {
    fn parse(s: &str) -> Option<Action> {
        Some(match s { "freeze" => Action::Freeze, "resume" => Action::Resume, "stop" => Action::Stop, "force_stop" => Action::ForceStop, _ => return None })
    }
    pub fn as_str(self) -> &'static str { match self { Action::Freeze => "freeze", Action::Resume => "resume", Action::Stop => "stop", Action::ForceStop => "force_stop" } }
    /// The signal name written to the audit log and shown on the History page.
    pub fn signal(self) -> &'static str { match self { Action::Freeze => "STOP", Action::Resume => "CONT", Action::Stop => "TERM", Action::ForceStop => "KILL" } }
    fn verb(self) -> &'static str { match self { Action::Freeze => "freeze", Action::Resume => "resume", Action::Stop => "stop", Action::ForceStop => "force stop" } }
}

fn signo(name: &str) -> i32 { match name { "STOP" => libc::SIGSTOP, "CONT" => libc::SIGCONT, "TERM" => libc::SIGTERM, _ => libc::SIGKILL } }

/// Machine-readable reason a process was not (or would not be) signalled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Code { Gone, PidReused, RootOwned, NotYours, Launchd, MonitorLizard, AlreadyFrozen, NotFrozen, Unverifiable, NotPreviewed, SignalFailed }

impl Code {
    fn reason(self, action: Option<Action>) -> String {
        match self {
            Code::Gone => format!("Already exited, nothing to {}", action.map_or("signal", Action::verb)),
            Code::PidReused => "This pid now belongs to a different process".into(),
            Code::RootOwned => "Owned by root, so it cannot be signalled from here".into(),
            Code::NotYours => "Owned by another user, so it cannot be signalled from here".into(),
            Code::Launchd => "launchd runs the whole system and is never signalled".into(),
            Code::MonitorLizard => "This is Monitor Lizard or part of its own process tree, so it is never signalled".into(),
            Code::AlreadyFrozen => "Already frozen: resume it instead".into(),
            Code::NotFrozen => "Not frozen, so there is nothing to resume".into(),
            Code::Unverifiable => "No start time or executable was recorded for this process, so it cannot be checked".into(),
            Code::NotPreviewed => "This process was not in the preview that was confirmed".into(),
            Code::SignalFailed => "The signal could not be sent".into(),
        }
    }
}

/// A refusal of the whole request, with a stable `code` for the page.
#[derive(Debug, PartialEq)]
pub struct ApiError { pub status: u16, pub code: &'static str, pub message: String }
fn fail(status: u16, code: &'static str, message: impl Into<String>) -> ApiError { ApiError { status, code, message: message.into() } }

/// One process of the request, as the collector captured it.
#[derive(Clone, Debug, PartialEq)]
pub struct Target { pub pid: u32, pub started: Option<u64>, pub exe: String }

#[derive(Debug, PartialEq)]
pub enum Mode { Preview, Commit { token: Option<String> } }

#[derive(Debug, PartialEq)]
pub struct Request { pub action: Action, pub scope: Scope, pub items: Vec<Target>, pub mode: Mode }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope { One, Chain }
impl Scope { fn as_str(self) -> &'static str { match self { Scope::One => "one", Scope::Chain => "chain" } } }

/// Every value of `name`, decoded. An item list may repeat `item`; nothing else may.
fn values(query: &str, name: &str) -> Vec<String> {
    query.split('&').filter_map(|kv| kv.split_once('=').or(Some((kv, ""))).filter(|(k, _)| *k == name).map(|(_, v)| crate::query::urldecode(v))).collect()
}

fn one(query: &str, name: &str) -> Result<Option<String>, ApiError> {
    let mut v = values(query, name);
    if v.len() > 1 { return Err(fail(400, "repeated_parameter", format!("{name} was given more than once"))); }
    Ok(v.pop())
}

/// `<pid>:<started>:<exe>`. The executable may itself contain colons, so only the first two split. An empty
/// `started` or `exe` is a process the collector could not read: it parses, and verification refuses it.
fn parse_item(s: &str) -> Result<Target, ApiError> {
    let bad = || fail(400, "bad_items", format!("item must be <pid>:<started>:<exe>, got {:?}", crate::watcher::visible(s)));
    let mut it = s.splitn(3, ':');
    let (pid, started, exe) = (it.next().ok_or_else(bad)?, it.next().ok_or_else(bad)?, it.next().ok_or_else(bad)?);
    let pid: u32 = pid.parse().map_err(|_| bad())?;
    let started = if started.is_empty() { None } else { Some(started.parse::<u64>().map_err(|_| bad())?) };
    if pid == 0 || exe.contains('\0') { return Err(bad()); }
    Ok(Target { pid, started, exe: exe.to_string() })
}

fn parse_items(query: &str) -> Result<Vec<Target>, ApiError> {
    let raw = values(query, "item");
    if raw.is_empty() { return Err(fail(400, "bad_items", "name at least one item")); }
    if raw.len() > MAX_ITEMS { return Err(fail(400, "too_many_items", format!("at most {MAX_ITEMS} processes at a time"))); }
    let items = raw.iter().map(|s| parse_item(s)).collect::<Result<Vec<_>, _>>()?;
    let mut pids: Vec<u32> = items.iter().map(|t| t.pid).collect();
    pids.sort_unstable();
    if pids.windows(2).any(|w| w[0] == w[1]) { return Err(fail(400, "bad_items", "a pid was named twice")); }
    Ok(items)
}

pub fn parse(query: &str) -> Result<Request, ApiError> {
    let action = one(query, "action")?.as_deref().and_then(Action::parse).ok_or_else(|| fail(400, "bad_action", "action must be freeze, resume, stop or force_stop"))?;
    let scope = match one(query, "scope")?.as_deref() { Some("one") => Scope::One, Some("chain") => Scope::Chain, _ => return Err(fail(400, "bad_scope", "scope must be one or chain")) };
    let items = parse_items(query)?;
    if scope == Scope::One && items.len() != 1 { return Err(fail(400, "bad_scope", "scope one takes exactly one item")); }
    // Only an explicit `dry=0` signals anything: a missing or misspelt value must not.
    let mode = match one(query, "dry")?.as_deref() {
        None | Some("1") => Mode::Preview,
        Some("0") => Mode::Commit { token: one(query, "confirm_token")? },
        Some(_) => return Err(fail(400, "bad_dry", "dry must be 1 (preview) or 0 (signal); leave it out for a preview")),
    };
    Ok(Request { action, scope, items, mode })
}

/// What the machine says about one pid right now.
#[derive(Clone, Debug, PartialEq)]
pub struct Fresh { pub uid: u32, pub state: char, pub exe: String, pub started: Option<u64> }

#[derive(Clone, Debug, PartialEq)]
pub struct Child { pub pid: u32, pub command: String, pub state: char }

/// Everything the route reads from or does to the machine, so a test can give it fakes.
pub trait World {
    fn probe(&self, pid: u32) -> Option<Fresh>;
    /// Direct children of `pid`.
    fn children(&self, pid: u32) -> Vec<Child>;
    fn send(&self, pid: u32, sig: i32) -> Result<(), String>;
    fn my_pid(&self) -> u32;
    fn my_uid(&self) -> u32;
    /// Parents and descendants of Monitor Lizard (the Tauri shell is one): never signalled.
    fn protected(&self) -> Vec<u32>;
}

fn is_frozen(f: &Fresh) -> bool { f.state == 'T' }

/// The fresh facts a pid passes verification with: it is the captured process, it is yours, and the action makes sense now.
fn verify(w: &dyn World, t: &Target, action: Option<Action>) -> Result<Fresh, Code> {
    if t.pid <= 1 { return Err(Code::Launchd); }
    if t.pid == w.my_pid() || w.protected().contains(&t.pid) { return Err(Code::MonitorLizard); }
    let Some(f) = w.probe(t.pid) else { return Err(Code::Gone) };
    if f.state == 'Z' { return Err(Code::Gone); }
    let (Some(started), false) = (t.started, t.exe.is_empty()) else { return Err(Code::Unverifiable) };
    if f.exe.is_empty() || f.started.is_none() { return Err(Code::Unverifiable); }
    if f.started != Some(started) || f.exe != t.exe { return Err(Code::PidReused); }
    if f.uid == 0 { return Err(Code::RootOwned); }
    if f.uid != w.my_uid() { return Err(Code::NotYours); }
    match action {
        Some(Action::Freeze) if is_frozen(&f) => Err(Code::AlreadyFrozen),
        Some(Action::Resume) if !is_frozen(&f) => Err(Code::NotFrozen),
        _ => Ok(f),
    }
}

/// The order signals go in: parents first to freeze or resume, children first to stop. Items arrive child first.
pub fn in_order(action: Action, items: &[Target]) -> Vec<Target> {
    let mut v = items.to_vec();
    if matches!(action, Action::Freeze | Action::Resume) { v.reverse(); }
    v
}

/// The signals one process gets. A stopped process holds a pending TERM until it is continued, so Stop on a
/// frozen one also sends CONT.
fn signals_for(action: Action, frozen: bool) -> Vec<&'static str> {
    match action { Action::Stop if frozen => vec!["TERM", "CONT"], a => vec![a.signal()] }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status { WouldSignal, Signalled, Refused, Failed }

#[derive(Serialize, Debug, Clone)]
pub struct ChildOut { pub pid: u32, pub command: String, pub frozen: bool }

#[derive(Serialize, Debug, Clone)]
pub struct ItemResult {
    pub pid: u32,
    pub status: Status,
    pub code: Option<Code>,
    pub reason: Option<String>,
    /// `running`, `frozen`, or null when the process is gone.
    pub state: Option<&'static str>,
    pub signals: Vec<&'static str>,
    pub children: Vec<ChildOut>,
    pub children_total: usize,
}

fn state_of(f: &Fresh) -> &'static str { if is_frozen(f) { "frozen" } else { "running" } }

fn kids(w: &dyn World, pid: u32) -> (Vec<ChildOut>, usize) {
    let all = w.children(pid);
    let total = all.len();
    (all.into_iter().take(CHILD_LIMIT).map(|c| ChildOut { pid: c.pid, command: c.command.chars().take(COMMAND_MAX).collect(), frozen: c.state == 'T' }).collect(), total)
}

fn refused(w: &dyn World, t: &Target, code: Code, action: Option<Action>, with_kids: bool) -> ItemResult {
    // The state of a refused process is still worth showing (a reused pid or a root-owned one is running).
    let state = match code { Code::Gone | Code::Launchd | Code::MonitorLizard | Code::Unverifiable => None, _ => w.probe(t.pid).map(|f| state_of(&f)) };
    let (children, children_total) = if with_kids && !matches!(code, Code::Gone | Code::PidReused) { kids(w, t.pid) } else { (vec![], 0) };
    ItemResult { pid: t.pid, status: Status::Refused, code: Some(code), reason: Some(code.reason(action)), state, signals: vec![], children, children_total }
}

fn check(w: &dyn World, t: &Target, action: Option<Action>, with_kids: bool) -> Result<ItemResult, ItemResult> {
    match verify(w, t, action) {
        Ok(f) => {
            let (children, children_total) = if with_kids { kids(w, t.pid) } else { (vec![], 0) };
            let signals = action.map(|a| signals_for(a, is_frozen(&f))).unwrap_or_default();
            Ok(ItemResult { pid: t.pid, status: Status::WouldSignal, code: None, reason: None, state: Some(state_of(&f)), signals, children, children_total })
        }
        Err(code) => Err(refused(w, t, code, action, with_kids)),
    }
}

/// What a confirmation is bound to: the action, the scope and every item with its captured start and executable.
fn binding(req: &Request) -> String {
    let mut parts: Vec<String> = req.items.iter().map(|t| format!("{}:{}:{}", t.pid, t.started.map_or(String::new(), |s| s.to_string()), t.exe)).collect();
    parts.sort();
    format!("{}|{}|{}", req.action.as_str(), req.scope.as_str(), parts.join("\n"))
}

#[derive(Serialize)]
struct Counts { would_signal: usize, signalled: usize, refused: usize, failed: usize }

#[derive(Serialize)]
struct Reply<'a> {
    ok: bool,
    dry_run: bool,
    action: &'static str,
    scope: &'static str,
    signal: &'static str,
    counts: Counts,
    items: &'a [ItemResult],
    confirm_token: Option<String>,
    confirm_token_expires_in_secs: Option<u64>,
    audit_error: Option<String>,
    note: &'static str,
}

/// Said in every reply.
pub const NOTE: &str = "Freeze (SIGSTOP) pauses a process until you resume it (SIGCONT); its children keep running. Stop (SIGTERM) asks a process to end, and Force stop (SIGKILL) ends it at once. Every action is recorded in History before it is sent.";

fn error_reply(e: &ApiError) -> (u16, String) {
    match serde_json::to_string(&serde_json::json!({ "ok": false, "error": e.message, "code": e.code })) {
        Ok(body) => (e.status, body),
        Err(_) => (500, r#"{"ok":false,"error":"could not build the error reply","code":"reply_failed"}"#.to_string()),
    }
}

fn render(req: &Request, items: &[ItemResult], dry: bool, token: Option<String>, audit_error: Option<String>) -> (u16, String) {
    let n = |s: Status| items.iter().filter(|i| i.status == s).count();
    let done = if dry { Status::WouldSignal } else { Status::Signalled };
    let reply = Reply { ok: n(done) == items.len() && audit_error.is_none(), dry_run: dry, action: req.action.as_str(), scope: req.scope.as_str(), signal: req.action.signal(),
        counts: Counts { would_signal: n(Status::WouldSignal), signalled: n(Status::Signalled), refused: n(Status::Refused), failed: n(Status::Failed) }, items,
        confirm_token_expires_in_secs: token.as_ref().map(|_| TOKEN_TTL.as_secs()), confirm_token: token, audit_error: audit_error.clone(), note: NOTE };
    let status = if audit_error.is_some() { 500 } else { 200 };
    match serde_json::to_string(&reply) {
        Ok(body) => (status, body),
        Err(e) => error_reply(&fail(500, "reply_failed", format!("the request ran but its reply could not be built ({e}); see History for what was sent"))),
    }
}

fn token_error(e: TokenError) -> ApiError {
    match e {
        TokenError::Missing => fail(400, "confirmation_required", "a real request needs the confirm_token from a preview of the same items"),
        TokenError::Unknown => fail(409, "confirmation_invalid", "that confirmation was not issued, or was already used; preview again"),
        TokenError::Expired => fail(409, "confirmation_expired", "that confirmation expired; preview again"),
        TokenError::OtherSelection => fail(409, "confirmation_mismatch", "that confirmation was issued for a different action or items; preview again"),
    }
}

/// Everything the signal route reads from the outside.
pub struct Ctx<'a> { pub tokens: &'a ConfirmTokens, pub db: &'a Path, pub world: &'a dyn World, pub now: Instant, pub unix_now: u64 }

pub fn reply(query: &str, ctx: &Ctx) -> (u16, String) {
    match handle(query, ctx) { Ok(r) => r, Err(e) => error_reply(&e) }
}

fn handle(query: &str, ctx: &Ctx) -> Result<(u16, String), ApiError> {
    let req = parse(query)?;
    let w = ctx.world;
    let ordered = in_order(req.action, &req.items);
    let bound = binding(&req);
    match &req.mode {
        Mode::Preview => {
            let items: Vec<ItemResult> = ordered.iter().map(|t| check(w, t, Some(req.action), true).unwrap_or_else(|r| r)).collect();
            let movable: Vec<String> = items.iter().filter(|i| i.status == Status::WouldSignal).map(|i| i.pid.to_string()).collect();
            let token = if movable.is_empty() { None } else {
                Some(ctx.tokens.issue(bound, movable, ctx.now).map_err(|e| match e {
                    IssueError::TooMany => fail(429, "too_many_previews", "too many previews are waiting for confirmation; use or wait out one of them"),
                    IssueError::NoRandom(m) => fail(500, "no_confirmation", m),
                })?)
            };
            Ok(render(&req, &items, true, token, None))
        }
        Mode::Commit { token } => {
            // The audit comes first: if it cannot be opened or written, nothing is signalled.
            let h = History::at(ctx.db).map_err(|e| fail(500, "audit_unavailable", format!("the audit log is unavailable, so nothing was signalled: {}", history::unavailable(&e))))?;
            let allowed = ctx.tokens.redeem(token.as_deref(), &bound, ctx.now).map_err(token_error)?;
            // First pass: what will be attempted. Only a process the preview showed as signalable can be.
            let mut results: Vec<(Target, Result<ItemResult, ItemResult>)> = ordered.iter().map(|t| {
                let r = match check(w, t, Some(req.action), false) {
                    Ok(_) if !allowed.contains(&t.pid.to_string()) => Err(refused_not_previewed(t, req.action)),
                    other => other,
                };
                (t.clone(), r)
            }).collect();
            let planned: Vec<u32> = results.iter().filter(|(_, r)| r.is_ok()).map(|(t, _)| t.pid).collect();
            let refused_now: Vec<serde_json::Value> = results.iter().filter_map(|(t, r)| r.as_ref().err().map(|e| refusal_json(t.pid, e))).collect();
            h.record_action(ctx.unix_now, req.action.signal(), &planned, &serde_json::to_string(&refused_now).unwrap_or_default())
                .map_err(|e| fail(500, "audit_unavailable", format!("the audit log could not be written, so nothing was signalled: {}", history::unavailable(&e))))?;
            // Second pass: each process is checked again immediately before its own signal.
            let mut late: Vec<serde_json::Value> = Vec::new();
            let mut items: Vec<ItemResult> = Vec::new();
            for (t, first) in results.drain(..) {
                let item = match first {
                    Err(e) => e,
                    Ok(_) => match check(w, &t, Some(req.action), false) {
                        Err(e) => { late.push(refusal_json(t.pid, &e)); e }
                        Ok(mut ok) => {
                            let mut sent = Vec::new();
                            let mut failure = None;
                            for name in ok.signals.clone() {
                                match w.send(t.pid, signo(name)) { Ok(()) => sent.push(name), Err(m) => { failure = Some(m); break; } }
                            }
                            ok.signals = sent;
                            match failure {
                                None => { ok.status = Status::Signalled; ok }
                                Some(m) => {
                                    ok.status = Status::Failed; ok.code = Some(Code::SignalFailed); ok.reason = Some(format!("{}: {m}", Code::SignalFailed.reason(None)));
                                    late.push(serde_json::json!({ "pid": t.pid, "reason": ok.reason }));
                                    ok
                                }
                            }
                        }
                    },
                };
                items.push(item);
            }
            let audit_error = if late.is_empty() { None } else {
                h.record_action(ctx.unix_now, req.action.signal(), &[], &serde_json::to_string(&late).unwrap_or_default()).err()
                    .map(|e| format!("a signal was refused or failed, and History could not record it: {}", history::unavailable(&e)))
            };
            Ok(render(&req, &items, false, None, audit_error))
        }
    }
}

fn refused_not_previewed(t: &Target, action: Action) -> ItemResult {
    ItemResult { pid: t.pid, status: Status::Refused, code: Some(Code::NotPreviewed), reason: Some(Code::NotPreviewed.reason(Some(action))), state: None, signals: vec![], children: vec![], children_total: 0 }
}

/// The shape the History page already reads in the `refused` column: `{ pid, reason }`.
fn refusal_json(pid: u32, e: &ItemResult) -> serde_json::Value { serde_json::json!({ "pid": pid, "reason": e.reason, "code": e.code }) }

/// `GET /api/requester/status?item=...`: each process's state right now, for the page's badges and buttons. It
/// signals nothing and writes nothing. `action` is optional; without it only identity, owner and exit are checked.
pub fn status_reply(query: &str, w: &dyn World) -> (u16, String) {
    let go = || -> Result<(u16, String), ApiError> {
        let items = parse_items(query)?;
        let action = match one(query, "action")?.as_deref() { None | Some("") => None, Some(a) => Some(Action::parse(a).ok_or_else(|| fail(400, "bad_action", "action must be freeze, resume, stop or force_stop"))?) };
        let out: Vec<ItemResult> = items.iter().map(|t| check(w, t, action, true).unwrap_or_else(|r| r)).collect();
        Ok((200, serde_json::to_string(&serde_json::json!({ "items": out })).unwrap_or_default()))
    };
    go().unwrap_or_else(|e| error_reply(&e))
}

// ---- the real machine

/// The real world: `ps`, libproc and `kill(2)`. The protected set is read once, when this is made, so build one per request.
pub struct Real { protected: Vec<u32> }

impl Real {
    pub fn new() -> Real {
        let table = process_table();
        Real { protected: related_to(std::process::id(), &table) }
    }
}
impl Default for Real { fn default() -> Self { Self::new() } }

/// `(pid, ppid)` for every process.
fn process_table() -> Vec<(u32, u32)> {
    let out = Command::new("ps").args(["-axo", "pid=,ppid="]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    out.lines().filter_map(|l| { let mut c = l.split_whitespace(); Some((c.next()?.parse().ok()?, c.next()?.parse().ok()?)) }).collect()
}

/// Every ancestor and every descendant of `me`, not `me` itself.
pub fn related_to(me: u32, table: &[(u32, u32)]) -> Vec<u32> {
    let parent: std::collections::HashMap<u32, u32> = table.iter().copied().collect();
    let mut out = Vec::new();
    let mut cur = parent.get(&me).copied();
    while let Some(p) = cur { if p <= 1 || out.contains(&p) { break; } out.push(p); cur = parent.get(&p).copied(); }
    let mut frontier = vec![me];
    while let Some(p) = frontier.pop() {
        for &(c, pp) in table { if pp == p && c != me && !out.contains(&c) { out.push(c); frontier.push(c); } }
    }
    out
}

/// `ps -o uid=,state=` for one pid: `(uid, state letter)`.
pub fn parse_probe(text: &str) -> Option<(u32, char)> {
    let mut c = text.split_whitespace();
    let uid = c.next()?.parse().ok()?;
    Some((uid, c.next()?.chars().next()?))
}

/// `ps -axww -o pid=,ppid=,state=,command=` rows whose parent is `of`.
pub fn parse_children(text: &str, of: u32) -> Vec<Child> {
    text.lines().filter_map(|l| {
        let mut it = l.trim_start().splitn(4, char::is_whitespace);
        let (pid, ppid, state) = (it.next()?.parse().ok()?, it.next()?.parse::<u32>().ok()?, it.next()?.chars().next()?);
        (ppid == of).then(|| Child { pid, state, command: it.next().unwrap_or("").trim().to_string() })
    }).collect()
}

impl World for Real {
    fn probe(&self, pid: u32) -> Option<Fresh> {
        let out = Command::new("ps").args(["-o", "uid=,state=", "-p", &pid.to_string()]).output().ok()?;
        let (uid, state) = parse_probe(&String::from_utf8_lossy(&out.stdout))?;
        let exe = i32::try_from(pid).ok().and_then(|p| libproc::proc_pid::pidpath(p).ok()).unwrap_or_default();
        Some(Fresh { uid, state, exe, started: crate::watcher::started_of(pid) })
    }
    fn children(&self, pid: u32) -> Vec<Child> {
        let out = Command::new("ps").args(["-axww", "-o", "pid=,ppid=,state=,command="]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        parse_children(&out, pid)
    }
    fn send(&self, pid: u32, sig: i32) -> Result<(), String> {
        let pid = i32::try_from(pid).map_err(|_| "pid out of range".to_string())?;
        if pid <= 1 { return Err("refusing pid 1 or below".into()); }
        // SAFETY: `kill(2)` takes plain integers and has no memory effects; `pid` is checked above to be a real, positive pid.
        if unsafe { libc::kill(pid, sig) } == 0 { Ok(()) } else { Err(std::io::Error::last_os_error().to_string()) }
    }
    fn my_pid(&self) -> u32 { std::process::id() }
    // SAFETY: getuid has no arguments and cannot fail.
    fn my_uid(&self) -> u32 { unsafe { libc::getuid() } }
    fn protected(&self) -> Vec<u32> { self.protected.clone() }
}

/// The route as the server runs it.
pub fn serve_reply(query: &str, tokens: &ConfirmTokens) -> (u16, String) {
    let unix_now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    reply(query, &Ctx { tokens, db: &history::db_path(), world: &Real::new(), now: Instant::now(), unix_now })
}

pub fn serve_status(query: &str) -> (u16, String) { status_reply(query, &Real::new()) }

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    const ME: u32 = 501;
    struct Fake { procs: RefCell<HashMap<u32, Fresh>>, kids: HashMap<u32, Vec<Child>>, sent: RefCell<Vec<(u32, i32)>>, protected: Vec<u32>, fail_send: Option<u32>, exit_after_send: Option<(u32, u32)> }
    impl Fake {
        fn new() -> Fake { Fake { procs: RefCell::new(HashMap::new()), kids: HashMap::new(), sent: RefCell::new(vec![]), protected: vec![], fail_send: None, exit_after_send: None } }
        fn with(self, pid: u32, uid: u32, state: char, exe: &str, started: u64) -> Fake { self.procs.borrow_mut().insert(pid, Fresh { uid, state, exe: exe.into(), started: Some(started) }); self }
        fn sent(&self) -> Vec<(u32, i32)> { self.sent.borrow().clone() }
    }
    impl World for Fake {
        fn probe(&self, pid: u32) -> Option<Fresh> { self.procs.borrow().get(&pid).cloned() }
        fn children(&self, pid: u32) -> Vec<Child> { self.kids.get(&pid).cloned().unwrap_or_default() }
        fn send(&self, pid: u32, sig: i32) -> Result<(), String> {
            if self.fail_send == Some(pid) { return Err("Operation not permitted".into()); }
            self.sent.borrow_mut().push((pid, sig));
            // Behave like the kernel, so a later check sees the effect.
            if let Some(f) = self.procs.borrow_mut().get_mut(&pid) { match sig { s if s == libc::SIGSTOP => f.state = 'T', s if s == libc::SIGCONT => f.state = 'S', s if s == libc::SIGKILL => f.state = 'Z', _ => {} } }
            // Something else exits while the request is part way through.
            if let Some((after, gone)) = self.exit_after_send { if after == pid { self.procs.borrow_mut().remove(&gone); } }
            Ok(())
        }
        fn my_pid(&self) -> u32 { 4242 }
        fn my_uid(&self) -> u32 { ME }
        fn protected(&self) -> Vec<u32> { self.protected.clone() }
    }
    fn t(pid: u32, started: u64, exe: &str) -> Target { Target { pid, started: Some(started), exe: exe.into() } }
    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ml-signal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn q(action: &str, scope: &str, items: &[(u32, u64, &str)], extra: &str) -> String {
        let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b == b'/' || b == b'.' || b == b'-' { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
        let it: String = items.iter().map(|(p, s, e)| format!("&item={}", enc(&format!("{p}:{s}:{e}")))).collect();
        format!("action={action}&scope={scope}{it}{extra}")
    }
    struct Rig { tokens: ConfirmTokens, db: std::path::PathBuf }
    impl Rig {
        fn new(name: &str) -> Rig { Rig { tokens: ConfirmTokens::default(), db: dir(name).join("h.db") } }
        fn call(&self, w: &dyn World, query: &str) -> (u16, serde_json::Value) {
            let (code, body) = reply(query, &Ctx { tokens: &self.tokens, db: &self.db, world: w, now: Instant::now(), unix_now: 100 });
            (code, serde_json::from_str(&body).expect("the reply is JSON"))
        }
        fn token(&self, w: &dyn World, query: &str) -> String {
            let (code, v) = self.call(w, query);
            assert_eq!(code, 200, "{v}");
            v["confirm_token"].as_str().unwrap_or_else(|| panic!("a preview that would signal something returns a token: {v}")).to_string()
        }
        fn actions(&self) -> Vec<(String, String, String)> {
            if !self.db.exists() { return vec![]; }
            History::at(&self.db).unwrap().recent_actions(100).unwrap().into_iter().map(|a| (a.sig, a.sent, a.refused)).collect()
        }
    }
    const EXE: &str = "/usr/bin/sleep";

    #[test]
    fn a_request_is_parsed_strictly() {
        let r = parse(&q("freeze", "one", &[(10, 5, "/a b/c:d,e")], "")).unwrap();
        assert_eq!((r.action, r.scope, r.mode), (Action::Freeze, Scope::One, Mode::Preview));
        assert_eq!(r.items, vec![t(10, 5, "/a b/c:d,e")], "an executable with a space, colon and comma survives");
        for (query, code) in [
            (q("pause", "one", &[(10, 5, EXE)], ""), "bad_action"), (q("freeze", "all", &[(10, 5, EXE)], ""), "bad_scope"),
            (q("freeze", "one", &[(10, 5, EXE), (11, 5, EXE)], ""), "bad_scope"), (q("freeze", "chain", &[], ""), "bad_items"),
            (q("freeze", "chain", &[(10, 5, EXE), (10, 6, EXE)], ""), "bad_items"), ("action=freeze&scope=one&item=abc".into(), "bad_items"),
            ("action=freeze&scope=one&item=0:5:/x".into(), "bad_items"), ("action=freeze&action=stop&scope=one&item=1:5:/x".into(), "repeated_parameter"),
            (q("freeze", "one", &[(10, 5, EXE)], "&dry=true"), "bad_dry"), (q("freeze", "one", &[(10, 5, EXE)], "&dry"), "bad_dry"),
        ] { assert_eq!(parse(&query).unwrap_err().code, code, "{query}"); }
        let many: Vec<(u32, u64, &str)> = (10..10 + MAX_ITEMS as u32 + 1).map(|p| (p, 5, EXE)).collect();
        assert_eq!(parse(&q("stop", "chain", &many, "")).unwrap_err().code, "too_many_items");
        assert_eq!(parse(&q("stop", "chain", &many[..MAX_ITEMS], "")).unwrap().items.len(), MAX_ITEMS);
        assert_eq!(parse(&q("stop", "one", &[(10, 5, EXE)], "&dry=0&confirm_token=abc")).unwrap().mode, Mode::Commit { token: Some("abc".into()) });
        assert_eq!(parse(&q("stop", "one", &[(10, 5, EXE)], "&dry=1")).unwrap().mode, Mode::Preview);
    }

    #[test]
    fn every_refusal_has_its_code_and_nothing_is_signalled() {
        let mut w = Fake::new().with(20, ME, 'S', EXE, 7).with(21, 0, 'S', "/usr/bin/login", 7).with(22, 999, 'S', EXE, 7).with(23, ME, 'T', EXE, 7)
            .with(24, ME, 'Z', EXE, 7).with(25, ME, 'S', EXE, 7).with(26, ME, 'S', EXE, 7).with(30, ME, 'S', EXE, 7);
        w.protected = vec![30];
        let case = |t: Target, a: Action| verify(&w, &t, Some(a)).err();
        assert_eq!(case(t(1, 7, "/sbin/launchd"), Action::Freeze), Some(Code::Launchd));
        assert_eq!(case(t(4242, 7, EXE), Action::Freeze), Some(Code::MonitorLizard), "itself");
        assert_eq!(case(t(30, 7, EXE), Action::Stop), Some(Code::MonitorLizard), "its parents and children");
        assert_eq!(case(t(99, 7, EXE), Action::Freeze), Some(Code::Gone), "exited");
        assert_eq!(case(t(24, 7, EXE), Action::Freeze), Some(Code::Gone), "a zombie has exited");
        assert_eq!(case(t(20, 8, EXE), Action::Freeze), Some(Code::PidReused), "same pid, other start time");
        assert_eq!(case(t(20, 7, "/bin/other"), Action::Freeze), Some(Code::PidReused), "same pid, other program");
        assert_eq!(case(t(21, 7, "/usr/bin/login"), Action::Freeze), Some(Code::RootOwned));
        assert_eq!(case(t(22, 7, EXE), Action::Stop), Some(Code::NotYours));
        assert_eq!(case(t(23, 7, EXE), Action::Freeze), Some(Code::AlreadyFrozen));
        assert_eq!(case(t(20, 7, EXE), Action::Resume), Some(Code::NotFrozen));
        assert_eq!(case(Target { pid: 20, started: None, exe: EXE.into() }, Action::Freeze), Some(Code::Unverifiable));
        assert_eq!(case(t(20, 7, ""), Action::Freeze), Some(Code::Unverifiable));
        assert!(case(t(20, 7, EXE), Action::Freeze).is_none() && case(t(23, 7, EXE), Action::Resume).is_none() && case(t(23, 7, EXE), Action::Stop).is_none());
        // Whole requests over the route: every code is in the reply and no signal is sent, preview or commit.
        let rig = Rig::new("refusals");
        let items = [(21, 7, "/usr/bin/login"), (22, 7, EXE), (99, 7, EXE), (20, 8, EXE)];
        let (code, v) = rig.call(&w, &q("freeze", "chain", &items, ""));
        assert_eq!(code, 200);
        let codes: Vec<&str> = v["items"].as_array().unwrap().iter().map(|i| i["code"].as_str().unwrap()).collect();
        assert_eq!(v["ok"], false);
        assert_eq!(v["confirm_token"], serde_json::Value::Null, "nothing to confirm");
        assert!(codes.contains(&"root_owned") && codes.contains(&"not_yours") && codes.contains(&"gone") && codes.contains(&"pid_reused"), "{codes:?}");
        assert!(v["items"].as_array().unwrap().iter().any(|i| i["reason"] == "Already exited, nothing to freeze"));
        assert!(v["items"].as_array().unwrap().iter().any(|i| i["reason"] == "This pid now belongs to a different process"));
        assert!(w.sent().is_empty() && rig.actions().is_empty());
    }

    #[test]
    fn chains_go_parent_first_to_freeze_and_child_first_to_stop() {
        let chain = [t(3, 1, "/c"), t(2, 1, "/b"), t(1000, 1, "/a")]; // child first, as the page lists them
        let pids = |a| in_order(a, &chain).iter().map(|x| x.pid).collect::<Vec<_>>();
        assert_eq!(pids(Action::Freeze), vec![1000, 2, 3]);
        assert_eq!(pids(Action::Resume), vec![1000, 2, 3]);
        assert_eq!(pids(Action::Stop), vec![3, 2, 1000]);
        assert_eq!(pids(Action::ForceStop), vec![3, 2, 1000]);
        // The same through the route: the order the signals were sent in.
        for (action, want) in [("freeze", vec![100, 101, 102]), ("force_stop", vec![102, 101, 100])] {
            let w = Fake::new().with(100, ME, 'S', EXE, 5).with(101, ME, 'S', EXE, 5).with(102, ME, 'S', EXE, 5);
            let rig = Rig::new(&format!("order-{action}"));
            let items = [(102, 5, EXE), (101, 5, EXE), (100, 5, EXE)];
            let tok = rig.token(&w, &q(action, "chain", &items, ""));
            let (code, v) = rig.call(&w, &q(action, "chain", &items, &format!("&dry=0&confirm_token={tok}")));
            assert_eq!((code, v["ok"].clone()), (200, true.into()), "{v}");
            assert_eq!(w.sent().iter().map(|s| s.0).collect::<Vec<_>>(), want, "{action}");
        }
    }

    #[test]
    fn a_preview_changes_nothing_and_a_commit_needs_its_own_single_use_token() {
        let w = Fake::new().with(10, ME, 'S', EXE, 5).with(11, ME, 'S', EXE, 5);
        let rig = Rig::new("token");
        let (code, v) = rig.call(&w, &q("freeze", "one", &[(10, 5, EXE)], ""));
        assert_eq!((code, v["dry_run"].clone(), v["counts"]["would_signal"].clone(), v["signal"].clone()), (200, true.into(), 1.into(), "STOP".into()));
        assert!(v["confirm_token"].as_str().unwrap().len() >= 32);
        assert!(w.sent().is_empty() && rig.actions().is_empty(), "a preview sends nothing and writes no audit row");
        let commit = |tok: &str, items: &[(u32, u64, &str)], action: &str| rig.call(&w, &q(action, if items.len() == 1 { "one" } else { "chain" }, items, &format!("&dry=0&confirm_token={tok}")));
        assert_eq!(rig.call(&w, &q("freeze", "one", &[(10, 5, EXE)], "&dry=0")).1["code"], "confirmation_required");
        assert_eq!(commit("nonsense", &[(10, 5, EXE)], "freeze").1["code"], "confirmation_invalid");
        // A token for one process does not freeze two, and a token for freeze does not stop.
        let tok = rig.token(&w, &q("freeze", "one", &[(10, 5, EXE)], ""));
        let (code, v) = commit(&tok, &[(10, 5, EXE), (11, 5, EXE)], "freeze");
        assert_eq!((code, v["code"].as_str()), (409, Some("confirmation_mismatch")));
        let tok = rig.token(&w, &q("freeze", "one", &[(10, 5, EXE)], ""));
        assert_eq!(commit(&tok, &[(10, 5, EXE)], "stop").1["code"], "confirmation_mismatch");
        assert!(w.sent().is_empty() && rig.actions().is_empty(), "every failed attempt left the process alone and wrote nothing");
        // The mismatching attempt used the token up; a fresh one works exactly once.
        let tok = rig.token(&w, &q("freeze", "one", &[(10, 5, EXE)], ""));
        let (code, v) = commit(&tok, &[(10, 5, EXE)], "freeze");
        assert_eq!((code, v["ok"].clone(), v["counts"]["signalled"].clone()), (200, true.into(), 1.into()));
        assert_eq!(w.sent(), vec![(10, libc::SIGSTOP)]);
        assert_eq!(commit(&tok, &[(10, 5, EXE)], "freeze").1["code"], "confirmation_invalid", "single use");
        assert_eq!(rig.actions(), vec![("STOP".to_string(), "[10]".to_string(), "[]".to_string())]);
    }

    #[test]
    fn nothing_is_signalled_when_the_audit_row_cannot_be_written() {
        let w = Fake::new().with(10, ME, 'S', EXE, 5);
        let rig = Rig::new("audit-down");
        let tok = rig.token(&w, &q("force_stop", "one", &[(10, 5, EXE)], ""));
        // A folder where the database should be: it exists but cannot be opened as one.
        let broken = Rig { tokens: ConfirmTokens::default(), db: dir("audit-dir") };
        let (code, v) = {
            let (c, b) = reply(&q("force_stop", "one", &[(10, 5, EXE)], &format!("&dry=0&confirm_token={tok}")), &Ctx { tokens: &rig.tokens, db: &broken.db, world: &w, now: Instant::now(), unix_now: 1 });
            (c, serde_json::from_str::<serde_json::Value>(&b).unwrap())
        };
        assert_eq!((code, v["code"].as_str()), (500, Some("audit_unavailable")));
        assert!(w.sent().is_empty(), "no audit, no signal");
    }

    #[test]
    fn a_process_that_changed_after_the_preview_is_refused_and_the_rest_still_go() {
        let w = Fake::new().with(10, ME, 'S', EXE, 5).with(11, ME, 'S', EXE, 5).with(12, ME, 'S', EXE, 5);
        let rig = Rig::new("late");
        let items = [(12, 5, EXE), (11, 5, EXE), (10, 5, EXE)];
        let tok = rig.token(&w, &q("stop", "chain", &items, ""));
        w.procs.borrow_mut().remove(&11); // exited between the preview and the click
        w.procs.borrow_mut().get_mut(&10).unwrap().started = Some(99); // pid reused
        let (code, v) = rig.call(&w, &q("stop", "chain", &items, &format!("&dry=0&confirm_token={tok}")));
        assert_eq!((code, v["ok"].clone()), (200, false.into()));
        let by: HashMap<u64, (String, String)> = v["items"].as_array().unwrap().iter().map(|i| (i["pid"].as_u64().unwrap(), (i["status"].as_str().unwrap().into(), i["code"].as_str().unwrap_or("").into()))).collect();
        assert_eq!(by[&12], ("signalled".into(), "".into()));
        assert_eq!(by[&11], ("refused".into(), "gone".into()));
        assert_eq!(by[&10], ("refused".into(), "pid_reused".into()));
        assert_eq!(w.sent(), vec![(12, libc::SIGTERM)]);
        let rows = rig.actions();
        assert!(rows.iter().all(|r| r.0 == "TERM"));
        assert!(rows.iter().any(|r| r.1 == "[12]" && r.2.contains("gone") && r.2.contains("pid_reused")), "the refusals are recorded in the row written before the signal: {rows:?}");
    }

    #[test]
    fn a_process_that_exits_part_way_is_refused_at_its_own_turn_and_recorded_after() {
        let mut w = Fake::new().with(10, ME, 'S', EXE, 5).with(11, ME, 'S', EXE, 5);
        w.exit_after_send = Some((11, 10)); // stop goes child first: 11 is signalled, then 10 disappears
        let rig = Rig::new("part-way");
        let items = [(11, 5, EXE), (10, 5, EXE)];
        let tok = rig.token(&w, &q("force_stop", "chain", &items, ""));
        let (code, v) = rig.call(&w, &q("force_stop", "chain", &items, &format!("&dry=0&confirm_token={tok}")));
        assert_eq!((code, v["ok"].clone(), v["items"][1]["code"].clone()), (200, false.into(), "gone".into()));
        assert_eq!(w.sent(), vec![(11, libc::SIGKILL)]);
        let rows = rig.actions();
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert!(rows[0].1 == "[]" && rows[0].2.contains("gone"), "the later row says what was refused at the last moment");
        assert_eq!(rows[1].1, "[11, 10]", "the first row, newest last, names everything that was about to be signalled");
    }

    #[test]
    fn something_that_was_not_signalable_at_preview_is_not_signalled_by_that_confirmation() {
        let w = Fake::new().with(10, ME, 'S', EXE, 5).with(11, ME, 'S', EXE, 5);
        let rig = Rig::new("not-previewed");
        w.procs.borrow_mut().get_mut(&11).unwrap().state = 'T'; // frozen at preview: a freeze refuses it
        let items = [(11, 5, EXE), (10, 5, EXE)];
        let tok = rig.token(&w, &q("freeze", "chain", &items, ""));
        w.procs.borrow_mut().get_mut(&11).unwrap().state = 'S'; // now it would pass, but it was not in the preview
        let (_, v) = rig.call(&w, &q("freeze", "chain", &items, &format!("&dry=0&confirm_token={tok}")));
        let c11 = v["items"].as_array().unwrap().iter().find(|i| i["pid"] == 11).unwrap().clone();
        assert_eq!(c11["code"], "not_previewed");
        assert_eq!(w.sent(), vec![(10, libc::SIGSTOP)]);
    }

    #[test]
    fn stopping_a_frozen_process_continues_it_so_the_term_arrives() {
        assert_eq!(signals_for(Action::Stop, true), vec!["TERM", "CONT"]);
        assert_eq!(signals_for(Action::Stop, false), vec!["TERM"]);
        assert_eq!(signals_for(Action::ForceStop, true), vec!["KILL"], "kill needs no continue");
        let w = Fake::new().with(10, ME, 'T', EXE, 5);
        let rig = Rig::new("stop-frozen");
        let tok = rig.token(&w, &q("stop", "one", &[(10, 5, EXE)], ""));
        let (_, v) = rig.call(&w, &q("stop", "one", &[(10, 5, EXE)], &format!("&dry=0&confirm_token={tok}")));
        assert_eq!(v["items"][0]["signals"], serde_json::json!(["TERM", "CONT"]));
        assert_eq!(w.sent(), vec![(10, libc::SIGTERM), (10, libc::SIGCONT)]);
    }

    #[test]
    fn a_failed_signal_is_reported_and_recorded() {
        let mut w = Fake::new().with(10, ME, 'S', EXE, 5);
        w.fail_send = Some(10);
        let rig = Rig::new("send-fails");
        let tok = rig.token(&w, &q("freeze", "one", &[(10, 5, EXE)], ""));
        let (code, v) = rig.call(&w, &q("freeze", "one", &[(10, 5, EXE)], &format!("&dry=0&confirm_token={tok}")));
        assert_eq!((code, v["ok"].clone(), v["items"][0]["status"].clone(), v["items"][0]["code"].clone()), (200, false.into(), "failed".into(), "signal_failed".into()));
        assert_eq!(rig.actions().len(), 2, "the attempt, then what went wrong");
    }

    #[test]
    fn the_status_route_reports_frozen_gone_reused_and_children_without_signalling() {
        let mut w = Fake::new().with(10, ME, 'T', EXE, 5).with(11, ME, 'S', EXE, 5);
        w.kids.insert(11, (0..20).map(|i| Child { pid: 500 + i, command: "x".repeat(400), state: 'S' }).collect());
        let (code, body) = status_reply(&q("", "one", &[(10, 5, EXE), (11, 5, EXE), (12, 5, EXE), (11, 6, EXE)].iter().take(3).cloned().collect::<Vec<_>>(), "").replacen("action=&scope=one&", "", 1), &w);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(code, 200);
        assert_eq!(v["items"][0]["state"], "frozen");
        assert_eq!((v["items"][1]["state"].clone(), v["items"][1]["children_total"].clone(), v["items"][1]["children"].as_array().unwrap().len()), ("running".into(), 20.into(), CHILD_LIMIT.into()));
        assert_eq!(v["items"][1]["children"][0]["command"].as_str().unwrap().chars().count(), COMMAND_MAX);
        assert_eq!(v["items"][2]["code"], "gone");
        assert!(w.sent().is_empty());
    }

    #[test]
    fn related_processes_are_every_parent_and_every_descendant() {
        // 1 launchd > 10 shell > 20 monitor-lizard > 30 child > 40 grandchild; 50 is unrelated.
        let table = [(1, 0), (10, 1), (20, 10), (30, 20), (40, 30), (50, 1)];
        let mut r = related_to(20, &table);
        r.sort();
        assert_eq!(r, vec![10, 30, 40]);
    }

    #[test]
    fn ps_output_is_read_defensively() {
        assert_eq!(parse_probe("  501 S+\n"), Some((501, 'S')));
        assert_eq!(parse_probe("0 Ts"), Some((0, 'T')));
        assert_eq!(parse_probe(""), None);
        assert_eq!(parse_probe("abc S"), None);
        let kids = parse_children("  10 1 S /sbin/launchd\n  11 10 T sleep 5\n  12 10 S node a.js --x\n bad\n", 10);
        assert_eq!(kids, vec![Child { pid: 11, state: 'T', command: "sleep 5".into() }, Child { pid: 12, state: 'S', command: "node a.js --x".into() }]);
    }

    // ---- real processes. Only processes this test starts are ever signalled, and each is killed at the end.

    /// A `sleep` the test started, detached from the test process (its parent is launchd), so Monitor
    /// Lizard's own-tree protection does not apply to it. Killed when dropped.
    struct Sleeper(u32);
    impl Sleeper {
        fn start() -> Sleeper {
            let out = Command::new("/bin/sh").args(["-c", "/bin/sleep 300 >/dev/null 2>&1 & echo $!"]).output().expect("start a sleep");
            let pid: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().expect("the sleep's pid");
            let s = Sleeper(pid);
            for _ in 0..50 { // The forked shell has not exec'd `sleep` yet until its path says so; its start time is only stable after that.
            if libproc::proc_pid::pidpath(pid as i32).is_ok_and(|p| p.ends_with("/sleep")) { break; } std::thread::sleep(std::time::Duration::from_millis(20)); }
            s
        }
        fn target(&self) -> (u32, u64, String) { (self.0, crate::watcher::started_of(self.0).expect("start time"), libproc::proc_pid::pidpath(self.0 as i32).expect("exe")) }
        fn state(&self) -> Option<char> { Real::new().probe(self.0).map(|f| f.state) }
        fn gone(&self) -> bool { for _ in 0..100 { if self.state().is_none_or(|s| s == 'Z') { return true; } std::thread::sleep(std::time::Duration::from_millis(20)); } false }
    }
    impl Drop for Sleeper { fn drop(&mut self) { unsafe { libc::kill(self.0 as i32, libc::SIGKILL); } } }

    #[test]
    fn real_sleep_processes_freeze_resume_and_stop_in_order() {
        let (a, b) = (Sleeper::start(), Sleeper::start());
        let (ta, tb) = (a.target(), b.target());
        let world = Real::new();
        let rig = Rig::new("real");
        let items = [(tb.0, tb.1, tb.2.as_str()), (ta.0, ta.1, ta.2.as_str())]; // b is the child
        let run = |action: &str, scope: &str, items: &[(u32, u64, &str)]| {
            let tok = rig.token(&world, &q(action, scope, items, ""));
            rig.call(&world, &q(action, scope, items, &format!("&dry=0&confirm_token={tok}")))
        };
        // Freeze one, see it frozen, and a second freeze is refused.
        let (code, v) = run("freeze", "one", &items[..1]);
        assert_eq!((code, v["ok"].clone()), (200, true.into()), "{v}");
        assert_eq!((b.state(), a.state()), (Some('T'), Some('S')), "only the chosen one froze");
        let (_, v) = rig.call(&world, &q("freeze", "one", &items[..1], ""));
        assert_eq!(v["items"][0]["code"], "already_frozen");
        assert_eq!(v["items"][0]["state"], "frozen");
        // Freeze the whole chain: b is already frozen, so only the chain's other process is.
        let (_, v) = rig.call(&world, &q("freeze", "chain", &items, ""));
        assert_eq!(v["counts"]["refused"], 1);
        // Resume the one, then freeze both together, then resume both.
        let (_, v) = run("resume", "one", &items[..1]);
        assert_eq!(v["ok"], true);
        assert_eq!(b.state(), Some('S'));
        let (_, v) = run("freeze", "chain", &items);
        assert_eq!(v["ok"], true, "{v}");
        assert_eq!((a.state(), b.state()), (Some('T'), Some('T')));
        let (_, v) = run("resume", "chain", &items);
        assert_eq!(v["ok"], true);
        assert_eq!((a.state(), b.state()), (Some('S'), Some('S')));
        // Stop a frozen process: the TERM is delivered because Stop also continues it.
        let (_, v) = run("freeze", "one", &items[..1]);
        assert_eq!(v["ok"], true);
        let (_, v) = run("stop", "one", &items[..1]);
        assert_eq!((v["ok"].clone(), v["items"][0]["signals"].clone()), (true.into(), serde_json::json!(["TERM", "CONT"])), "{v}");
        assert!(b.gone(), "the frozen process ended");
        // A gone process is refused now, and the other is force stopped.
        let (_, v) = rig.call(&world, &q("stop", "one", &items[..1], ""));
        assert_eq!(v["items"][0]["code"], "gone");
        let (_, v) = run("force_stop", "one", &items[1..]);
        assert_eq!(v["ok"], true);
        assert!(a.gone());
        let rows = rig.actions();
        let sigs: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
        assert!(sigs.contains(&"STOP") && sigs.contains(&"CONT") && sigs.contains(&"TERM") && sigs.contains(&"KILL"), "{sigs:?}");
    }

    #[test]
    fn real_pid_reuse_is_refused_and_the_process_is_left_alone() {
        let a = Sleeper::start();
        let (pid, started, exe) = a.target();
        let world = Real::new();
        let rig = Rig::new("real-reuse");
        for items in [[(pid, started + 5, exe.as_str())], [(pid, started, "/bin/other")]] {
            let (_, v) = rig.call(&world, &q("force_stop", "one", &items, ""));
            assert_eq!((v["items"][0]["code"].clone(), v["confirm_token"].clone()), ("pid_reused".into(), serde_json::Value::Null));
        }
        assert_eq!(a.state(), Some('S'));
        assert!(rig.actions().is_empty());
    }

    #[test]
    fn real_launchd_and_this_test_process_are_never_signalled() {
        let world = Real::new();
        let rig = Rig::new("real-protected");
        let me = std::process::id();
        let (_, v) = rig.call(&world, &q("force_stop", "chain", &[(1, 1, "/sbin/launchd"), (me, 1, "/x")], ""));
        let codes: Vec<&str> = v["items"].as_array().unwrap().iter().map(|i| i["code"].as_str().unwrap()).collect();
        assert_eq!(codes, vec!["launchd", "monitor_lizard"], "listed in the order given: stop goes child first");
        assert_eq!(v["confirm_token"], serde_json::Value::Null);
    }

    #[test]
    fn real_a_child_of_this_process_is_refused_as_part_of_its_own_tree() {
        let mut child = Command::new("/bin/sleep").arg("300").spawn().expect("start a child");
        std::thread::sleep(std::time::Duration::from_millis(100));
        let pid = child.id();
        let (started, exe) = (crate::watcher::started_of(pid).unwrap(), libproc::proc_pid::pidpath(pid as i32).unwrap());
        let world = Real::new();
        let (_, v) = Rig::new("real-child").call(&world, &q("stop", "one", &[(pid, started, exe.as_str())], ""));
        assert_eq!(v["items"][0]["code"], "monitor_lizard");
        assert_eq!(Real::new().probe(pid).map(|f| f.state).filter(|s| *s != 'Z'), Some('S'), "still running");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn real_a_root_owned_process_is_refused_by_its_owner() {
        // launchd's own children include root daemons; any pid owned by root with a readable path will do. Nothing is signalled:
        // this only previews, and the fake-free check stops at the owner.
        let table = process_table();
        let world = Real::new();
        let root_pid = table.iter().map(|&(p, _)| p).filter(|&p| p > 1).find(|&p| world.probe(p).is_some_and(|f| f.uid == 0 && !f.exe.is_empty() && f.started.is_some()));
        let Some(pid) = root_pid else { return };
        let f = world.probe(pid).unwrap();
        let (_, v) = Rig::new("real-root").call(&world, &q("force_stop", "one", &[(pid, f.started.unwrap(), f.exe.as_str())], ""));
        assert_eq!(v["items"][0]["code"], "root_owned");
        assert_eq!(v["confirm_token"], serde_json::Value::Null);
    }
}
