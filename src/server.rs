use crate::{web, actions, baseline::{self, Kind}, classify, history, history::History, llm, llm_server, persistence, privilege, procusage, requester_signal, sampler, settings, snapshot, storage, trash_route, watcher, query::{self, urldecode}};
use std::sync::{Arc, Mutex};
use std::{thread, time::Duration};
use tiny_http::{Header, Method, Response, Server};

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
/// Removal requests `/api/history` returns, newest first; kept apart from the kill log's limit.
const TRASH_LIMIT: usize = 200;
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
fn token() -> String { query::random_token().expect("random token") }

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
    let web_files = web::files(&secret);
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
    // Rows an earlier run left half-written say so now, before any request can add rows of its own.
    if let Some(h) = history::open_or_report() {
        match h.reconcile_interrupted_trash() {
            Ok(0) => {}
            Ok(n) => eprintln!("monitor-lizard: {n} Trash audit row(s) were left unfinished by an earlier run and are marked interrupted"),
            Err(e) => eprintln!("monitor-lizard: could not check the Trash audit for unfinished rows: {}", history::unavailable(&e)),
        }
    }
    let storage_cache: Arc<storage::Cache> = Default::default();
    let confirmations: Arc<trash_route::ConfirmTokens> = Default::default();
    let signal_confirmations: trash_route::ConfirmTokens = Default::default();
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
        let write_ok = write_allowed(&origins, hdr("Origin").as_deref(), &secret, hdr("X-MonitorLizard-Token").as_deref());
        let json_resp = |body: String| Response::from_string(body).with_header(Header::from_bytes("content-type", "application/json").unwrap()).boxed();
        let cached = |c: &Cache| json_resp(c.lock().unwrap().clone().unwrap_or_else(|| "null".into()));
        let forbidden = || Response::from_string("forbidden").with_status_code(403).boxed();
        // Removal can take a while (re-measuring each item), so it runs on its own thread: the dashboard
        // and Stop requests are not held up behind it. Only one runs at a time.
        if host_ok && write_ok && *req.method() == Method::Post && path == "/api/trash" {
            let query = query.to_string();
            let (cache, tokens) = (storage_cache.clone(), confirmations.clone());
            thread::spawn(move || {
                let (code, body) = match trash_route::Busy::try_begin() {
                    Some(_busy) => trash_route::serve_reply(&query, &cache, &tokens),
                    None => trash_route::busy_reply(),
                };
                let res = Response::from_string(body).with_status_code(code).with_header(Header::from_bytes("content-type", "application/json").unwrap());
                let _ = req.respond(res);
            });
            continue;
        }
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
            (Method::Get, "/api/history") => match History::open().and_then(|h| Ok((h.seen_rows(SEEN_LIMIT)?, h.recent_actions(ACTION_LIMIT)?, h.recent_baseline_changes(BASELINE_CHANGE_LIMIT)?, h.recent_trash_moves(TRASH_LIMIT)?))) {
                Ok((seen, actions, baseline_changes, trash_moves)) => {
                    // This route needs no token, so removal paths are shown relative to home; the database keeps the full ones.
                    let home = storage::home_or_reason().ok();
                    let trash_moves: Vec<_> = trash_moves.into_iter().map(|m| match &home { Some(h) => m.with_home_as_tilde(h), None => m }).collect();
                    match serde_json::to_string(&serde_json::json!({ "seen": seen, "actions": actions, "baseline_changes": baseline_changes, "trash_moves": trash_moves })) {
                        Ok(body) => json_resp(body),
                        Err(e) => Response::from_string(format!("could not build the history reply: {e}")).with_status_code(500).boxed(),
                    }
                }
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
            // Preview and confirm, like /api/trash: nothing is signalled without a token from a preview of the same items.
            (Method::Post, "/api/requester/signal") if write_ok => { let (code, body) = requester_signal::serve_reply(query, &signal_confirmations); json_resp(body).with_status_code(code) }
            // Command lines of other processes: the token is required even for this read (the module decides).
            (Method::Get, "/api/requester/status") => { let (code, body) = requester_signal::serve_status(write_ok, query); json_resp(body).with_status_code(code) }
            (Method::Post, _) => forbidden(),
            (Method::Get, "/fonts/IBMPlexSans-Regular.woff2") => Response::from_data(FONT_REGULAR).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexSans-Medium.woff2") => Response::from_data(FONT_MEDIUM).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexSans-SemiBold.woff2") => Response::from_data(FONT_SEMIBOLD).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexMono-Regular.woff2") => Response::from_data(MONO_REGULAR).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexMono-Medium.woff2") => Response::from_data(MONO_MEDIUM).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexMono-SemiBold.woff2") => Response::from_data(MONO_SEMIBOLD).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            // Last, so no /api path ever reaches the file lookup.
            (Method::Get, _) => web::reply(&web_files, &hosts, hdr("Host").as_deref(), hdr("Sec-Fetch-Site").as_deref(), path),
            _ => Response::from_string("not found").with_status_code(404).boxed(),
        };
        let _ = req.respond(res);
    }
}

fn unix_now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

/// The capacity series and its forecast. It only reads: a read-only connection, no table creation, no
/// migration. A database that cannot be read gives `available: false` instead of an error, so the page
/// still shows the current capacity.
fn storage_history_reply(db: &std::path::Path, now: u64) -> serde_json::Value {
    let series = history::read_capacity_series(db, now).map_err(|e| history::unavailable(&e));
    storage::forecast::reply(series, history::local_day(now, history::local_utc_offset(now)))
}

/// A write needs the same origin (when the browser sends one) and the page's per-run secret.
fn write_allowed(origins: &[String], origin: Option<&str>, secret: &str, token: Option<&str>) -> bool {
    origin.is_none_or(|o| origins.iter().any(|x| x == o)) && token.is_some_and(|t| query::secrets_equal(t, secret))
}

/// The DNS-rebinding guard: the Host header must be exactly one of the addresses this server listens on.
pub(crate) fn host_allowed(hosts: &[String], header: Option<&str>) -> bool { header.is_some_and(|h| hosts.iter().any(|x| x == h)) }

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
    fn a_write_needs_the_per_run_token_and_a_matching_origin() {
        let origins = ["http://127.0.0.1:4747".to_string(), "http://localhost:4747".to_string()];
        assert!(write_allowed(&origins, None, "s3cret", Some("s3cret")), "no Origin header (curl, the shell) with the token passes");
        assert!(write_allowed(&origins, Some("http://localhost:4747"), "s3cret", Some("s3cret")));
        for (o, t) in [(None, None), (None, Some("wrong")), (None, Some("")), (Some("http://evil.example"), Some("s3cret")), (Some("http://127.0.0.1:4748"), Some("s3cret")), (Some("null"), Some("s3cret")), (Some("http://localhost:4747"), None)] {
            assert!(!write_allowed(&origins, o, "s3cret", t), "{o:?} {t:?}");
        }
    }

    #[test]
    fn only_the_servers_own_host_names_pass_the_host_check() {
        let hosts = ["127.0.0.1:4747".to_string(), "localhost:4747".to_string()];
        assert!(host_allowed(&hosts, Some("127.0.0.1:4747")) && host_allowed(&hosts, Some("localhost:4747")));
        for bad in [Some("evil.example:4747"), Some("127.0.0.1"), Some("127.0.0.1:4748"), Some(""), None] { assert!(!host_allowed(&hosts, bad), "{bad:?}"); }
    }
}
