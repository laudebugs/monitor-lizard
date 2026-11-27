use crate::{actions, baseline::{self, Kind}, classify, history, history::History, llm, llm_server, persistence, privilege, procusage, sampler, settings, snapshot, storage, watcher};
use std::sync::{Arc, Mutex};
use std::{thread, time::Duration};
use tiny_http::{Header, Method, Response, Server};

/// Every file of the dashboard, embedded so the binary stays one offline file. The order of the scripts
/// is the load order the shell's `<script>` tags use (tests/web/split.test.mjs checks the two agree).
/// Nothing is read from disk at request time: a path not listed here is a 404.
const WEB_SOURCES: &[(&str, &str, &str)] = &[
    ("/", HTML, include_str!("../web/index.html")),
    ("/app.css", CSS, include_str!("../web/app.css")),
    ("/js/core.js", JS, include_str!("../web/core.js")),
    ("/js/table.js", JS, include_str!("../web/table.js")),
    ("/js/data.js", JS, include_str!("../web/data.js")),
    ("/js/page-overview.js", JS, include_str!("../web/page-overview.js")),
    ("/js/page-apps.js", JS, include_str!("../web/page-apps.js")),
    ("/js/page-privilege.js", JS, include_str!("../web/page-privilege.js")),
    ("/js/page-listeners.js", JS, include_str!("../web/page-listeners.js")),
    ("/js/page-persistence.js", JS, include_str!("../web/page-persistence.js")),
    ("/js/page-network.js", JS, include_str!("../web/page-network.js")),
    ("/js/page-history.js", JS, include_str!("../web/page-history.js")),
    ("/js/page-performance.js", JS, include_str!("../web/page-performance.js")),
    ("/js/page-storage.js", JS, include_str!("../web/page-storage.js")),
    ("/js/page-guide.js", JS, include_str!("../web/page-guide.js")),
    ("/js/helpers.js", JS, include_str!("../web/helpers.js")),
    ("/js/shell.js", JS, include_str!("../web/shell.js")),
];
const HTML: &str = "text/html; charset=utf-8";
const CSS: &str = "text/css; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";
/// The one file that holds `const TOKEN = "__TOKEN__"`. The token goes in that script only, never in the
/// shell, a URL or the hash.
const TOKEN_FILE: &str = "/js/core.js";

struct WebFile { path: &'static str, content_type: &'static str, body: String }

/// The dashboard's files with this run's token put in the script that declares it. Like the page before
/// the split, no `cache-control` is sent: the files live in the binary, so a new binary serves new ones.
fn web_files(secret: &str) -> Vec<WebFile> {
    WEB_SOURCES.iter().map(|&(path, content_type, body)| WebFile {
        path,
        content_type,
        body: if path == TOKEN_FILE { body.replace("__TOKEN__", secret) } else { body.to_string() },
    }).collect()
}

fn web_response(f: &WebFile) -> tiny_http::ResponseBox {
    Response::from_string(f.body.clone()).with_header(Header::from_bytes("content-type", f.content_type).unwrap()).boxed()
}
// Bundled so the UI's type pairing works with nothing installed (ADR 0002: still no network request).
// IBM Plex Sans, SIL Open Font License 1.1 (web/fonts/OFL.txt), subsetted to basic Latin plus common
// punctuation; two static weights instantiated from the upstream variable font.
const FONT_REGULAR: &[u8] = include_bytes!("../web/fonts/IBMPlexSans-Regular.woff2");
const FONT_MEDIUM: &[u8] = include_bytes!("../web/fonts/IBMPlexSans-Medium.woff2");
const FONT_SEMIBOLD: &[u8] = include_bytes!("../web/fonts/IBMPlexSans-SemiBold.woff2");
const MONO_REGULAR: &[u8] = include_bytes!("../web/fonts/IBMPlexMono-Regular.woff2");
const MONO_MEDIUM: &[u8] = include_bytes!("../web/fonts/IBMPlexMono-Medium.woff2");
const MONO_SEMIBOLD: &[u8] = include_bytes!("../web/fonts/IBMPlexMono-SemiBold.woff2");
/// How many rows `/api/history` returns: newest first, enough for the History page.
const SEEN_LIMIT: usize = 2000;
/// Live sudo/su rows read to explain the last two hours of authorization failures.
const LIVE_JOIN_LIMIT: usize = 500;
const ACTION_LIMIT: usize = 200;
const BASELINE_CHANGE_LIMIT: usize = 200;
/// Shorter than `llm::classify`'s own 20 s default: see the comment at its call site in `/api/classify`.
const CLASSIFY_TIMEOUT: Duration = Duration::from_secs(6);

type Cache = Arc<Mutex<Option<String>>>;

/// Recompute `f` every `every` and cache its JSON, so history accumulates with no browser open.
fn refresh<T: serde::Serialize + 'static>(cache: &Cache, every: Duration, f: fn() -> T) {
    let cache = cache.clone();
    thread::spawn(move || loop {
        let json = serde_json::to_string(&f()).unwrap_or_default();
        *cache.lock().unwrap() = Some(json);
        thread::sleep(every);
    });
}

/// Per-run secret embedded in the page; required on every write so a stray request can't act.
fn token() -> String {
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut b)).expect("random token");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn query_param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| urldecode(v)))
}

/// Binds a free loopback port without serving yet, so a caller that cannot panic at startup (a Tauri
/// shell with a tray to show an error in) can bind, check the `Result`, and decide what to do before
/// any request loop starts; `serve` below is just this plus its own fixed port and the `expect`.
pub fn bind_free() -> Result<(Arc<Server>, u16), String> {
    // Bind loopback only, same as `serve`: this tool exposes everything about the machine.
    let server = Server::http("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = server.server_addr().to_ip().ok_or_else(|| "not an IP listener".to_string())?.port();
    Ok((Arc::new(server), port))
}

pub fn serve(port: u16) {
    // Bind loopback only: this tool exposes everything about the machine.
    let addr = format!("127.0.0.1:{port}");
    let server = Arc::new(Server::http(&addr).expect("bind failed"));
    println!("monitor-lizard -> http://{addr}");
    serve_on(server);
}

/// The request loop on an already-bound server, shared by `serve` (a fixed port) and a shell that
/// bound its own free port with `bind_free`: every guard, cache and background thread below is the
/// same either way, so there is exactly one place that wires them up.
pub fn serve_on(server: Arc<Server>) {
    let bound = server.server_addr().to_ip();
    let addr = bound.map(|a| a.to_string()).unwrap_or_default();
    let port = bound.map(|a| a.port()).unwrap_or(0);
    let secret = token();
    let web = web_files(&secret);
    let (snap, privs, pers): (Cache, Cache, Cache) = (Default::default(), Default::default(), Default::default());
    refresh(&snap, Duration::from_secs(10), snapshot::take);
    refresh(&privs, Duration::from_secs(30), || {
        // The live-capture rows explain failed authorizations whose process is already gone.
        let live = History::open().and_then(|h| h.recent_sudo(LIVE_JOIN_LIMIT)).unwrap_or_else(|e| {
            // Say so: otherwise every failed authorization silently reads as "no capture".
            eprintln!("monitor-lizard: could not read the sudo history to explain authorization failures: {}", history::unavailable(&e));
            Vec::new()
        });
        privilege::recent(120, &live)
    });
    {
        let pers = pers.clone();
        thread::spawn(move || loop { persistence::refresh_cache(&pers); thread::sleep(Duration::from_secs(60)); });
    }
    // Say at startup if the history cannot be opened (for instance a database from a newer build), instead of showing empty pages.
    drop(history::open_or_report());
    let storage_cache: Arc<storage::Cache> = Default::default();
    let storage_scan: storage::ScanFn = Arc::new(storage::scan);
    let metrics = sampler::start();
    let processes = procusage::start(actions::my_username());
    history::start_capacity_writer();
    let agent = llm_server::start();
    history::start_metrics_writer(metrics.clone(), || settings::load().metrics_history);
    thread::spawn(|| {
        let h = history::open_or_report();
        watcher::watch(|e| { if let Some(h) = &h { let _ = h.record_sudo(&e); } })
    });
    let origins = [format!("http://{addr}"), format!("http://localhost:{port}")];
    let hosts = [addr.clone(), format!("localhost:{port}")];
    for req in server.incoming_requests() {
        let url = req.url().to_string();
        let (path, query) = url.split_once('?').unwrap_or((&url, ""));
        let hdr = |n: &'static str| req.headers().iter().find(|h| h.field.equiv(n)).map(|h| h.value.to_string());
        let host_ok = host_allowed(&hosts, hdr("Host").as_deref()); // DNS-rebinding guard
        // Writes need: same origin (if sent) and the page's secret token.
        let write_ok = hdr("Origin").is_none_or(|o| origins.contains(&o)) && hdr("X-MonitorLizard-Token").is_some_and(|t| t == secret);
        let web_file = web.iter().find(|f| f.path == path);
        let json_resp = |body: String| Response::from_string(body).with_header(Header::from_bytes("content-type", "application/json").unwrap()).boxed();
        let cached = |c: &Cache| json_resp(c.lock().unwrap().clone().unwrap_or_else(|| "null".into()));
        let forbidden = || Response::from_string("forbidden").with_status_code(403).boxed();
        let res = match (req.method(), path) {
            _ if !host_ok => Response::from_string("bad host").with_status_code(403).boxed(),
            (Method::Get, "/api/snapshot") if query == "fresh=1" => json_resp(serde_json::to_string(&snapshot::take()).unwrap_or_default()),
            (Method::Get, "/api/snapshot") => cached(&snap),
            (Method::Get, "/api/privilege") => cached(&privs),
            (Method::Get, "/api/persistence") => cached(&pers),
            (Method::Get, "/api/storage") => json_resp(storage_cache.respond(query, &storage_scan)),
            (Method::Get, "/api/storage/history") => json_resp(storage_history_reply(&history::db_path(), unix_now()).to_string()),
            (Method::Get, "/api/volume") => { let (code, body) = storage::boot_capacity_reply(); json_resp(body).with_status_code(code) }
            (Method::Get, "/api/metrics") => {
                let since = query_param(query, "since").and_then(|s| s.parse().ok()).unwrap_or(0);
                match query_param(query, "res").as_deref() {
                    None | Some("raw") => json_resp(serde_json::to_string(&sampler::since(&metrics, since)).unwrap_or_default()),
                    Some(res) => json_resp(serde_json::to_string(&History::open().and_then(|h| h.query_metrics(res, since)).unwrap_or_default()).unwrap_or_default()),
                }
            }
            (Method::Get, "/api/processes") => {
                let rows = processes.read().map(|r| r.clone()).unwrap_or_default();
                // `pids=` picks out specific rows (the Stop dialog's "fetch one fresh row before
                // opening", for one or every pid in a Quit-the-app group) and ignores sort/limit
                // entirely, since there is no "top" ordering to apply to a caller's own exact list.
                match query_param(query, "pids") {
                    Some(raw) => {
                        let wanted: std::collections::HashSet<u32> = raw.split(',').filter_map(|s| s.parse().ok()).collect();
                        let picked: Vec<_> = rows.into_iter().filter(|r| wanted.contains(&r.pid)).collect();
                        json_resp(serde_json::to_string(&picked).unwrap_or_default())
                    }
                    None => {
                        let sort = query_param(query, "sort").unwrap_or_else(|| "cpu".into());
                        let limit = query_param(query, "limit").and_then(|s| s.parse().ok()).unwrap_or(50);
                        json_resp(serde_json::to_string(&procusage::sort_and_limit(rows, &sort, limit)).unwrap_or_default())
                    }
                }
            }
            (Method::Get, "/api/history") => match History::open().and_then(|h| Ok((h.seen_rows(SEEN_LIMIT)?, h.recent_actions(ACTION_LIMIT)?, h.recent_baseline_changes(BASELINE_CHANGE_LIMIT)?, h.recent_trash_moves(ACTION_LIMIT)?))) {
                Ok((seen, actions, baseline_changes, trash_moves)) => json_resp(serde_json::json!({ "seen": seen, "actions": actions, "baseline_changes": baseline_changes, "trash_moves": trash_moves }).to_string()),
                // An unreadable database must not look like "no history".
                Err(e) => Response::from_string(history::unavailable(&e)).with_status_code(500).boxed(),
            }
            (Method::Get, "/api/sudo") => match History::open().and_then(|h| h.recent_sudo(50)) {
                Ok(v) => json_resp(serde_json::to_string(&v).unwrap_or_default()),
                Err(e) => Response::from_string(history::unavailable(&e)).with_status_code(500).boxed(),
            }
            (Method::Get, "/api/guide") => json_resp(crate::guide::to_json()),
            (Method::Get, "/api/settings") => json_resp(serde_json::to_string(&settings::load()).unwrap_or_default()),
            (Method::Post, "/api/settings") if write_ok => {
                match settings::update(query_param(query, "notifications").as_deref(), query_param(query, "generic_text").as_deref(), query_param(query, "quiet_start").as_deref(), query_param(query, "quiet_end").as_deref(), query_param(query, "metrics_history").as_deref()) {
                    Ok(s) => json_resp(serde_json::to_string(&s).unwrap_or_default()),
                    Err(e) => Response::from_string(e).with_status_code(400).boxed(),
                }
            }
            (Method::Get, "/api/baseline") => { let (b, notice) = baseline::peek(); json_resp(serde_json::json!({ "entries": b.entries(), "notice": notice }).to_string()) }
            (Method::Post, "/api/approve" | "/api/unapprove") if write_ok => match apply_change(query, path == "/api/approve") {
                Ok(c) => {
                    if c.needs_startup_rescan() { refresh_startup_cache(&pers, &c.key, c.mark); }
                    Response::from_string(if c.changed { "ok" } else { "unchanged" }).boxed()
                }
                Err((code, msg)) => Response::from_string(msg).with_status_code(code).boxed(),
            }
            (Method::Post, "/api/classify") if write_ok => {
                let key = query_param(query, "group_key").unwrap_or_default();
                let snap = snapshot::take();
                match snap.groups.iter().find(|g| g.key == key) {
                    Some(g) => {
                        let allowed = classify::allowed_actions(g.trust, g);
                        let base_url = agent.base_url_if_ready();
                        // A second, independent snapshot inside `evidence` is the established CLI
                        // pattern (see `classify` in main.rs); the group disappearing between the
                        // two is rare and distinct from the agent not being ready, so it gets its
                        // own reason rather than borrowing the agent's.
                        // Bounded below llm::classify's own 20 s default: this server handles one
                        // request at a time, so a slow-but-alive model server would otherwise stall
                        // every other open dashboard tab for as long as this one call waits.
                        let raw = match (&base_url, classify::evidence(&key)) {
                            (Some(url), Some(ev)) => llm::classify_with_timeout(url, &ev, CLASSIFY_TIMEOUT),
                            (Some(_), None) => Err(llm::LlmError::Invalid("group vanished between snapshots".into())),
                            (None, _) => Err(llm::LlmError::Unavailable(agent.reason().as_str().to_string())),
                        };
                        let agent_reason = base_url.is_none().then(|| agent.reason().as_str());
                        let verdict = classify::validate_verdict(g.trust, &allowed, raw);
                        // `rule_label`/`allowed_actions` alongside the clamped verdict, per the todo's
                        // own Scope line: a caller needs the rules' own answer, not just what the
                        // agent did or didn't add to it.
                        json_resp(serde_json::json!({ "rule_label": g.trust, "allowed_actions": allowed, "verdict": verdict, "agent_reason": agent_reason }).to_string())
                    }
                    None => Response::from_string("no such group").with_status_code(404).boxed(),
                }
            }
            (Method::Post, "/api/kill") if write_ok => {
                match actions::parse(&query_param(query, "pids").unwrap_or_default(), &query_param(query, "start").unwrap_or_default(), &query_param(query, "sig").unwrap_or_default()) {
                    Ok((items, sig)) => {
                        let o = actions::execute(&items, sig);
                        if let Ok(h) = History::open() {
                            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                            let _ = h.record_action(now, sig.name(), &o.sent, &serde_json::to_string(&o.refused).unwrap_or_default());
                        }
                        json_resp(serde_json::to_string(&o).unwrap_or_default())
                    }
                    Err(e) => Response::from_string(e).with_status_code(400).boxed(),
                }
            }
            (Method::Post, "/api/trash") if write_ok => {
                let (code, body) = trash_reply(query, &storage_cache);
                json_resp(body).with_status_code(code)
            }
            (Method::Post, _) => forbidden(),
            (Method::Get, "/fonts/IBMPlexSans-Regular.woff2") => Response::from_data(FONT_REGULAR).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexSans-Medium.woff2") => Response::from_data(FONT_MEDIUM).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexSans-SemiBold.woff2") => Response::from_data(FONT_SEMIBOLD).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexMono-Regular.woff2") => Response::from_data(MONO_REGULAR).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexMono-Medium.woff2") => Response::from_data(MONO_MEDIUM).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexMono-SemiBold.woff2") => Response::from_data(MONO_SEMIBOLD).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, _) if web_file.is_some() => web_response(web_file.unwrap()),
            _ => Response::from_string("not found").with_status_code(404).boxed(),
        };
        let _ = req.respond(res);
    }
}

/// `POST /api/trash?ids=a,b[&dry=1][&confirm_yours=1]`. A real move needs the audit log to be writable,
/// so a request that cannot be recorded moves nothing.
fn trash_reply(query: &str, cache: &storage::Cache) -> (u16, String) {
    let err = |code, msg: String| (code, serde_json::json!({ "error": msg }).to_string());
    let ids = match storage::trash::parse_ids(&query_param(query, "ids").unwrap_or_default()) { Ok(v) => v, Err(e) => return err(400, e) };
    let items = match cache.find(&ids) { Ok(v) => v, Err(e) => return err(400, e) };
    let Some(home) = storage::home() else { return err(400, storage::reason::HOME_UNKNOWN.into()) };
    let dry = query_param(query, "dry").as_deref() == Some("1");
    let confirm_yours = query_param(query, "confirm_yours").as_deref() == Some("1");
    if dry {
        let o = storage::trash::run(&items, &home, std::path::Path::new("/Applications"), confirm_yours, true, &actions::trash);
        return (200, serde_json::to_string(&o).unwrap_or_default());
    }
    let h = match History::open() { Ok(h) => h, Err(e) => return err(500, format!("the audit log is unavailable, so nothing was moved: {}", history::unavailable(&e))) };
    let o = storage::trash::run(&items, &home, std::path::Path::new("/Applications"), confirm_yours, false, &actions::trash);
    let audit = h.record_trash_move(unix_now(), &o);
    let mut v = serde_json::to_value(&o).unwrap_or_default();
    if let Err(e) = audit { v["audit_error"] = e.to_string().into(); }
    (200, v.to_string())
}

fn unix_now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

/// The capacity series and its forecast. It only reads: a read-only connection, no table creation, no
/// migration. A database that cannot be read gives `available: false` instead of an error, so the page
/// still shows the current capacity.
fn storage_history_reply(db: &std::path::Path, now: u64) -> serde_json::Value {
    let series = history::read_capacity_series(db, now).map_err(|e| history::unavailable(&e));
    storage::forecast::reply(series, history::local_day(now, history::local_utc_offset(now)))
}

/// The DNS-rebinding guard: the Host header must be exactly one of the addresses this server listens on.
fn host_allowed(hosts: &[String], header: Option<&str>) -> bool { header.is_some_and(|h| hosts.iter().any(|x| x == h)) }

/// The outcome of a mark or unmark request that passed validation.
struct Change { kind: Kind, key: String, mark: bool, changed: bool }

impl Change {
    /// Only a real change to a startup item can alter the cached list, so a repeated request costs nothing.
    fn needs_startup_rescan(&self) -> bool { self.changed && self.kind == Kind::Startup }
}

/// Marks or unmarks one item: `kind` is `app` (the default, as before kinds existed), `host`, `startup`
/// or `finding`. A host is given as `app` and `addr` (or a ready `key`), so the page never builds keys.
fn apply_change(query: &str, mark: bool) -> Result<Change, (u16, String)> {
    let kind_name = query_param(query, "kind").unwrap_or_else(|| "app".into());
    let Some(kind) = Kind::parse(&kind_name) else { return Err((400, format!("unknown kind {kind_name:?}: use app, host, startup or finding"))) };
    let key = match (kind, query_param(query, "app"), query_param(query, "addr")) {
        (Kind::Host, Some(app), Some(addr)) => baseline::host_key(&app, &addr),
        _ => query_param(query, "key").unwrap_or_default(),
    };
    match baseline::change(kind, &key, mark) {
        Ok(changed) => Ok(Change { kind, key, mark, changed }),
        Err(baseline::ChangeError::Rejected(m)) => Err((400, m)),
        Err(baseline::ChangeError::Failed(m)) => Err((500, m)),
    }
}

/// Updates the cached startup list at once so the next read shows the change, then rescans in the
/// background to correct anything the patch cannot know (such as an item becoming new again).
fn refresh_startup_cache(cache: &Cache, key: &str, expected: bool) {
    if let Some(json) = cache.lock().unwrap().as_mut() { *json = persistence::patch_expected(json, key, expected); }
    static RESCANNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if RESCANNING.swap(true, std::sync::atomic::Ordering::SeqCst) { return; }
    let cache = cache.clone();
    thread::spawn(move || {
        persistence::refresh_cache(&cache);
        RESCANNING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => { out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(b'?')); i += 3; }
            b'+' => { out.push(b' '); i += 1 }
            c => { out.push(c); i += 1 }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeated_mark_triggers_no_startup_rescan_and_a_real_one_does() {
        let c = |kind, changed| Change { kind, key: "/p.plist".into(), mark: true, changed };
        assert!(!c(Kind::Startup, false).needs_startup_rescan(), "a no-op must not scan");
        assert!(c(Kind::Startup, true).needs_startup_rescan());
        assert!(!c(Kind::Host, true).needs_startup_rescan(), "other kinds never touch the startup list");
    }

    #[test]
    fn requests_with_a_bad_kind_or_key_are_400_before_anything_changes() {
        assert_eq!(apply_change("kind=planet&key=x", true).err().map(|e| e.0), Some(400));
        assert_eq!(apply_change("kind=host&key=nopipe", true).err().map(|e| e.0), Some(400));
        assert_eq!(apply_change("kind=app&key=", true).err().map(|e| e.0), Some(400));
        assert_eq!(apply_change("kind=finding&key=%2FA.app%3Aunsigned-network%23x", true).err().map(|e| e.0), Some(400));
    }

    /// A data: URI in an attribute may not contain a raw double quote or angle bracket: the first quote
    /// ends the attribute and the rest of the SVG is parsed as page markup, leaving stray text on screen.
    #[test]
    fn favicon_href_stays_inside_its_attribute() {
        let page = WEB_SOURCES[0].2;
        let at = page.find("rel=\"icon\"").expect("icon link");
        let after_href = page[at..].split_once("href=\"").expect("href attribute").1;
        let value = after_href.split('"').next().unwrap();
        assert!(value.starts_with("data:image/svg+xml,"), "value: {value}");
        assert!(!value.contains('<') && !value.contains('>'), "raw angle bracket in href: {value}");
        assert!(value.ends_with("%3C/svg%3E"), "href was cut short: {value}");
        assert!(after_href[value.len()..].starts_with("\">"), "attribute does not close the tag");
    }

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn the_token_is_put_into_the_script_that_declares_it_and_nowhere_else() {
        let files = web_files(SECRET);
        for f in &files {
            assert!(!f.body.contains("__TOKEN__"), "{} still holds the placeholder", f.path);
            let n = f.body.matches(SECRET).count();
            if f.path == TOKEN_FILE { assert_eq!(n, 1, "{} must carry the token once", f.path); } else { assert_eq!(n, 0, "{} must not carry the token", f.path); }
        }
        let core = files.iter().find(|f| f.path == TOKEN_FILE).unwrap();
        assert!(core.body.contains(&format!("const TOKEN = \"{SECRET}\";")));
        // The unreplaced sources keep the placeholder in the token file only, so a file moved there by mistake shows up.
        let with_placeholder: Vec<_> = WEB_SOURCES.iter().filter(|s| s.2.contains("__TOKEN__")).map(|s| s.0).collect();
        assert_eq!(with_placeholder, [TOKEN_FILE]);
    }

    #[test]
    fn every_file_in_web_is_served_and_every_served_file_exists() {
        let mut on_disk: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/web")).unwrap()
            .filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".js") || n.ends_with(".css") || n.ends_with(".html")).collect();
        on_disk.sort();
        let mut listed: Vec<String> = WEB_SOURCES.iter().map(|s| if s.0 == "/" { "index.html".to_string() } else { s.0.rsplit('/').next().unwrap().to_string() }).collect();
        listed.sort();
        assert_eq!(on_disk, listed, "a file in web/ that is not in WEB_SOURCES would 404 in the binary");
    }

    #[test]
    fn the_shell_loads_exactly_the_listed_files_in_the_listed_order() {
        let shell = WEB_SOURCES[0].2;
        let attr = |prefix: &str| -> Vec<String> { shell.split(prefix).skip(1).filter_map(|r| r.split('"').next().map(String::from)).collect() };
        let scripts = attr("<script src=\"");
        let listed: Vec<String> = WEB_SOURCES.iter().filter(|s| s.1 == JS).map(|s| s.0.to_string()).collect();
        assert_eq!(scripts, listed);
        assert_eq!(scripts.first().map(String::as_str), Some(TOKEN_FILE), "core.js loads first");
        assert_eq!(attr("<link rel=\"stylesheet\" href=\""), ["/app.css"]);
    }

    /// One request over a real socket to a server running the same Host check and file lookup as
    /// `serve_on`, without its sampler and history threads (they write under the real home folder).
    fn fetch(host: &str, path: &str) -> (u16, String, String) {
        use std::io::{Read, Write};
        let (server, port) = bind_free().expect("bind");
        let hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
        let files = web_files(SECRET);
        let worker = Arc::clone(&server);
        let handle = thread::spawn(move || {
            let req = worker.incoming_requests().next().unwrap();
            let host_ok = host_allowed(&hosts, req.headers().iter().find(|h| h.field.equiv("Host")).map(|h| h.value.to_string()).as_deref());
            let url = req.url().to_string();
            let res = match files.iter().find(|f| f.path == url) {
                _ if !host_ok => Response::from_string("bad host").with_status_code(403).boxed(),
                Some(f) => web_response(f),
                None => Response::from_string("not found").with_status_code(404).boxed(),
            };
            let _ = req.respond(res);
        });
        let host = host.replace("PORT", &port.to_string());
        let mut conn = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(conn, "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").unwrap();
        let mut raw = String::new();
        conn.read_to_string(&mut raw).unwrap();
        handle.join().unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").unwrap();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, head.to_ascii_lowercase(), body.to_string())
    }

    #[test]
    fn each_web_file_is_served_with_its_content_type_and_no_cache_header() {
        for &(path, content_type, _) in WEB_SOURCES {
            let (status, head, body) = fetch("127.0.0.1:PORT", path);
            assert_eq!(status, 200, "{path}");
            assert!(head.contains(&format!("content-type: {}", content_type.to_ascii_lowercase())), "{path}: {head}");
            assert!(!head.contains("cache-control"), "{path} must match the page, which sends none: {head}");
            assert!(!body.is_empty(), "{path}");
        }
    }

    #[test]
    fn unknown_paths_are_404_and_the_host_check_still_guards_every_file() {
        for path in ["/nope.js", "/js/", "/js/missing.js", "/index.html", "/../Cargo.toml", "/js/../Cargo.toml", "/web/core.js"] {
            assert_eq!(fetch("127.0.0.1:PORT", path).0, 404, "{path}");
        }
        for path in ["/", "/app.css", "/js/core.js"] {
            let (status, _, body) = fetch("evil.example:PORT", path);
            assert_eq!((status, body.as_str()), (403, "bad host"), "{path} with a foreign Host");
            assert!(!body.contains(SECRET));
        }
        // The token is the secret of the page's own origin: a rebinding page must not read the script that holds it.
        assert_eq!(fetch("localhost:PORT", "/js/core.js").0, 200);
    }

    #[test]
    fn bind_free_picks_a_real_port_and_a_second_call_picks_a_different_one() {
        let (_s1, p1) = bind_free().expect("first bind");
        let (_s2, p2) = bind_free().expect("second bind");
        assert_ne!(p1, 0, "the OS must hand back the port it actually chose, not the ask-for-any-port 0");
        assert_ne!(p2, 0);
        assert_ne!(p1, p2, "two independent binds must not collide on the same free port");
    }

    /// `serve_on`'s own request loop is `server.incoming_requests()`; this exercises the same
    /// `Arc<Server>::unblock` a Quit menu item calls, directly on a bound server, so the shutdown
    /// path is covered without also starting every background thread `serve_on` spawns for it.
    #[test]
    fn unblock_ends_the_request_loop_instead_of_blocking_forever() {
        let (server, _port) = bind_free().expect("bind");
        let loop_server = Arc::clone(&server);
        let handle = thread::spawn(move || {
            for _ in loop_server.incoming_requests() {}
        });
        server.unblock();
        // If unblock() did not do its job, this join() hangs rather than failing outright --
        // tiny_http's own documented contract for unblock() is what makes that an acceptable risk here.
        handle.join().expect("the request loop thread must exit once unblocked, not panic");
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ml-server-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_history_route_reads_the_series_without_changing_the_database() {
        let dir = temp_dir("history-route");
        let db = dir.join("h.db");
        let day = 86_400u64;
        {
            let h = History::at(&db).unwrap();
            for i in 0..8u64 { h.record_capacity(100 * day + i * day + 3600, 0, ((600 + 10 * i) * 1_000_000_000, 0, (400 - 10 * i) * 1_000_000_000)).unwrap(); }
        }
        let before = std::fs::read(&db).unwrap();
        let v = storage_history_reply(&db, 108 * day);
        assert_eq!(v["available"], true);
        assert_eq!(v["samples"].as_array().unwrap().len(), 8);
        assert!(v["forecast"]["status"].is_string());
        assert_eq!(std::fs::read(&db).unwrap(), before, "a GET must not write to the database file");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_history_route_says_unavailable_for_an_unreadable_database_and_empty_for_a_missing_one() {
        let dir = temp_dir("history-unavailable");
        // A folder where the database should be: it exists but cannot be opened as one.
        let v = storage_history_reply(&dir, 1_000_000);
        assert_eq!((v["available"].clone(), v["forecast"]["status"].clone()), (false.into(), "unavailable".into()));
        assert!(v["reason"].as_str().unwrap().contains("history unavailable"));
        let v = storage_history_reply(&dir.join("none.db"), 1_000_000);
        assert_eq!((v["available"].clone(), v["forecast"]["status"].clone()), (true.into(), "collecting".into()));
        assert!(!dir.join("none.db").exists(), "reading does not create the file");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn only_the_servers_own_host_names_pass_the_host_check() {
        let hosts = ["127.0.0.1:4747".to_string(), "localhost:4747".to_string()];
        assert!(host_allowed(&hosts, Some("127.0.0.1:4747")) && host_allowed(&hosts, Some("localhost:4747")));
        for bad in [Some("evil.example:4747"), Some("127.0.0.1"), Some("127.0.0.1:4748"), Some(""), None] { assert!(!host_allowed(&hosts, bad), "{bad:?}"); }
    }
}
