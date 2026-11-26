use crate::{actions, baseline::{self, Kind}, history, history::History, persistence, privilege, procusage, sampler, settings, snapshot, watcher};
use std::sync::{Arc, Mutex};
use std::{thread, time::Duration};
use tiny_http::{Header, Method, Response, Server};

const PAGE: &str = include_str!("../web/index.html");
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
const ACTION_LIMIT: usize = 200;
const BASELINE_CHANGE_LIMIT: usize = 200;

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

pub fn serve(port: u16) {
    // Bind loopback only: this tool exposes everything about the machine.
    let addr = format!("127.0.0.1:{port}");
    let server = Server::http(&addr).expect("bind failed");
    println!("monitor-lizard -> http://{addr}");
    let secret = token();
    let page = PAGE.replace("__TOKEN__", &secret);
    let (snap, privs, pers): (Cache, Cache, Cache) = (Default::default(), Default::default(), Default::default());
    refresh(&snap, Duration::from_secs(10), snapshot::take);
    refresh(&privs, Duration::from_secs(30), || privilege::recent(120));
    refresh(&pers, Duration::from_secs(60), persistence::take);
    // Say at startup if the history cannot be opened (for instance a database from a newer build), instead of showing empty pages.
    drop(history::open_or_report());
    let metrics = sampler::start();
    let processes = procusage::start(actions::my_username());
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
        let host_ok = hdr("Host").is_some_and(|h| hosts.contains(&h)); // DNS-rebinding guard
        // Writes need: same origin (if sent) and the page's secret token.
        let write_ok = hdr("Origin").is_none_or(|o| origins.contains(&o)) && hdr("X-MonitorLizard-Token").is_some_and(|t| t == secret);
        let json_resp = |body: String| Response::from_string(body).with_header(Header::from_bytes("content-type", "application/json").unwrap()).boxed();
        let cached = |c: &Cache| json_resp(c.lock().unwrap().clone().unwrap_or_else(|| "null".into()));
        let forbidden = || Response::from_string("forbidden").with_status_code(403).boxed();
        let res = match (req.method(), path) {
            _ if !host_ok => Response::from_string("bad host").with_status_code(403).boxed(),
            (Method::Get, "/api/snapshot") if query == "fresh=1" => json_resp(serde_json::to_string(&snapshot::take()).unwrap_or_default()),
            (Method::Get, "/api/snapshot") => cached(&snap),
            (Method::Get, "/api/privilege") => cached(&privs),
            (Method::Get, "/api/persistence") => cached(&pers),
            (Method::Get, "/api/metrics") => {
                let since = query_param(query, "since").and_then(|s| s.parse().ok()).unwrap_or(0);
                match query_param(query, "res").as_deref() {
                    None | Some("raw") => json_resp(serde_json::to_string(&sampler::since(&metrics, since)).unwrap_or_default()),
                    Some(res) => json_resp(serde_json::to_string(&History::open().and_then(|h| h.query_metrics(res, since)).unwrap_or_default()).unwrap_or_default()),
                }
            }
            (Method::Get, "/api/processes") => {
                let sort = query_param(query, "sort").unwrap_or_else(|| "cpu".into());
                let limit = query_param(query, "limit").and_then(|s| s.parse().ok()).unwrap_or(50);
                let rows = processes.read().map(|r| r.clone()).unwrap_or_default();
                json_resp(serde_json::to_string(&procusage::sort_and_limit(rows, &sort, limit)).unwrap_or_default())
            }
            (Method::Get, "/api/history") => match History::open().and_then(|h| Ok((h.seen_rows(SEEN_LIMIT)?, h.recent_actions(ACTION_LIMIT)?, h.recent_baseline_changes(BASELINE_CHANGE_LIMIT)?))) {
                Ok((seen, actions, baseline_changes)) => json_resp(serde_json::json!({ "seen": seen, "actions": actions, "baseline_changes": baseline_changes }).to_string()),
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
            (Method::Post, "/api/kill") if write_ok => {
                match actions::parse(&query_param(query, "pids").unwrap_or_default(), &query_param(query, "sig").unwrap_or_default()) {
                    Ok((pids, sig)) => {
                        let o = actions::execute(&pids, sig);
                        if let Ok(h) = History::open() {
                            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                            let _ = h.record_action(now, sig.name(), &o.sent, &serde_json::to_string(&o.refused).unwrap_or_default());
                        }
                        json_resp(serde_json::to_string(&o).unwrap_or_default())
                    }
                    Err(e) => Response::from_string(e).with_status_code(400).boxed(),
                }
            }
            (Method::Post, _) => forbidden(),
            (Method::Get, "/fonts/IBMPlexSans-Regular.woff2") => Response::from_data(FONT_REGULAR).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexSans-Medium.woff2") => Response::from_data(FONT_MEDIUM).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexSans-SemiBold.woff2") => Response::from_data(FONT_SEMIBOLD).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexMono-Regular.woff2") => Response::from_data(MONO_REGULAR).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexMono-Medium.woff2") => Response::from_data(MONO_MEDIUM).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/fonts/IBMPlexMono-SemiBold.woff2") => Response::from_data(MONO_SEMIBOLD).with_header(Header::from_bytes("content-type", "font/woff2").unwrap()).with_header(Header::from_bytes("cache-control", "public, max-age=31536000, immutable").unwrap()).boxed(),
            (Method::Get, "/") => Response::from_string(page.clone()).with_header(Header::from_bytes("content-type", "text/html; charset=utf-8").unwrap()).boxed(),
            _ => Response::from_string("not found").with_status_code(404).boxed(),
        };
        let _ = req.respond(res);
    }
}

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
fn refresh_startup_cache(cache: &Cache, path: &str, expected: bool) {
    if let Some(json) = cache.lock().unwrap().as_mut() { *json = persistence::patch_expected(json, path, expected); }
    static RESCANNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if RESCANNING.swap(true, std::sync::atomic::Ordering::SeqCst) { return; }
    let cache = cache.clone();
    thread::spawn(move || {
        *cache.lock().unwrap() = Some(serde_json::to_string(&persistence::take()).unwrap_or_default());
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
        let at = PAGE.find("rel=\"icon\"").expect("icon link");
        let after_href = PAGE[at..].split_once("href=\"").expect("href attribute").1;
        let value = after_href.split('"').next().unwrap();
        assert!(value.starts_with("data:image/svg+xml,"), "value: {value}");
        assert!(!value.contains('<') && !value.contains('>'), "raw angle bracket in href: {value}");
        assert!(value.ends_with("%3C/svg%3E"), "href was cut short: {value}");
        assert!(after_href[value.len()..].starts_with("\">"), "attribute does not close the tag");
    }
}
