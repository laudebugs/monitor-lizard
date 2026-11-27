//! The dashboard's files, embedded so the binary stays one offline file, and the one function that
//! decides who may fetch them. `server::serve_on` and the tests both go through `reply`, so a guard
//! cannot be missing from the real route while a test passes.
use std::io::Cursor;
use tiny_http::{Header, Response, ResponseBox, StatusCode};

pub struct Source { pub path: &'static str, pub content_type: &'static str, pub text: &'static str }

const HTML: &str = "text/html; charset=utf-8";
const CSS: &str = "text/css; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";

/// The order of the scripts is the load order the shell's `<script>` tags use (tests/web/split.test.mjs
/// and a test below check the two agree). Nothing is read from disk at request time: a path not listed
/// here is a 404.
pub const SOURCES: &[Source] = &[
    Source { path: "/", content_type: HTML, text: include_str!("../web/index.html") },
    Source { path: "/app.css", content_type: CSS, text: include_str!("../web/app.css") },
    Source { path: "/js/core.js", content_type: JS, text: include_str!("../web/core.js") },
    Source { path: "/js/table.js", content_type: JS, text: include_str!("../web/table.js") },
    Source { path: "/js/data.js", content_type: JS, text: include_str!("../web/data.js") },
    Source { path: "/js/page-overview.js", content_type: JS, text: include_str!("../web/page-overview.js") },
    Source { path: "/js/page-apps.js", content_type: JS, text: include_str!("../web/page-apps.js") },
    Source { path: "/js/page-privilege.js", content_type: JS, text: include_str!("../web/page-privilege.js") },
    Source { path: "/js/page-listeners.js", content_type: JS, text: include_str!("../web/page-listeners.js") },
    Source { path: "/js/page-persistence.js", content_type: JS, text: include_str!("../web/page-persistence.js") },
    Source { path: "/js/page-network.js", content_type: JS, text: include_str!("../web/page-network.js") },
    Source { path: "/js/page-history.js", content_type: JS, text: include_str!("../web/page-history.js") },
    Source { path: "/js/page-performance.js", content_type: JS, text: include_str!("../web/page-performance.js") },
    Source { path: "/js/page-storage.js", content_type: JS, text: include_str!("../web/page-storage.js") },
    Source { path: "/js/page-guide.js", content_type: JS, text: include_str!("../web/page-guide.js") },
    Source { path: "/js/helpers.js", content_type: JS, text: include_str!("../web/helpers.js") },
    Source { path: "/js/page-storage-logic.js", content_type: JS, text: include_str!("../web/page-storage-logic.js") },
    Source { path: "/js/shell.js", content_type: JS, text: include_str!("../web/shell.js") },
];

/// The per-run secret lives in the shell only, as `<meta name="ml-token" content="__TOKEN__">`. A script
/// would be a fixed URL any website could load with a `<script src>` and read the secret from, because
/// classic scripts share one global scope; the shell's body cannot be read across origins.
const TOKEN_FILE: &str = "/";

pub struct File { pub path: &'static str, pub content_type: &'static str, pub body: &'static str }

/// The files with this run's token put into the shell. The one replaced shell lives for the whole run,
/// so it is leaked once at startup and every request serves a `&'static str` without copying it.
pub fn files(secret: &str) -> Vec<File> {
    SOURCES.iter().map(|s| File {
        path: s.path,
        content_type: s.content_type,
        body: if s.path == TOKEN_FILE { Box::leak(s.text.replace("__TOKEN__", secret).into_boxed_str()) } else { s.text },
    }).collect()
}

/// A page on another site can name any of these URLs in a `<script>` or `<link>`. Browsers say where a
/// request came from in `Sec-Fetch-Site`; the header is absent from curl and some older web views, which
/// stay allowed, and `same-origin` and `none` (typing the address) are this page and the owner.
fn cross_site(header: Option<&str>) -> bool { matches!(header, Some("cross-site" | "same-site")) }

fn status(code: u16, text: &str) -> ResponseBox { Response::from_string(text).with_status_code(code).boxed() }

/// The answer for a GET that no API route took: a dashboard file, a 403 or a 404. `hosts` are the
/// server's own names (DNS-rebinding guard); the page carries a per-run secret, so nothing is cached.
pub fn reply(files: &[File], hosts: &[String], host: Option<&str>, sec_fetch_site: Option<&str>, path: &str) -> ResponseBox {
    if !crate::server::host_allowed(hosts, host) { return status(403, "bad host"); }
    let Some(f) = files.iter().find(|f| f.path == path) else { return status(404, "not found") };
    if f.content_type != HTML && cross_site(sec_fetch_site) { return status(403, "forbidden"); }
    let header = |k: &str, v: &str| Header::from_bytes(k, v).unwrap();
    Response::new(
        StatusCode(200),
        vec![header("content-type", f.content_type), header("cache-control", "no-store"), header("x-content-type-options", "nosniff")],
        Cursor::new(f.body.as_bytes()),
        Some(f.body.len()),
        None,
    ).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn the_token_is_in_the_shells_meta_tag_and_in_no_other_file() {
        let all = files(SECRET);
        for f in &all {
            assert!(!f.body.contains("__TOKEN__"), "{} still holds the placeholder", f.path);
            let n = f.body.matches(SECRET).count();
            if f.path == "/" { assert_eq!(n, 1, "the shell carries the token once"); } else { assert_eq!(n, 0, "{} must not carry the token", f.path); }
        }
        let shell = all[0].body;
        assert!(shell.contains(&format!("<meta name=\"ml-token\" content=\"{SECRET}\">")), "the token sits in the meta tag only");
        let with_placeholder: Vec<_> = SOURCES.iter().filter(|s| s.text.contains("__TOKEN__")).map(|s| s.path).collect();
        assert_eq!(with_placeholder, ["/"], "the placeholder is in the shell's source and nowhere else");
        assert_eq!(SOURCES[0].text.matches("__TOKEN__").count(), 1);
    }

    #[test]
    fn every_file_in_web_is_served_and_every_served_file_exists() {
        let mut on_disk: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/web")).unwrap()
            .filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".js") || n.ends_with(".css") || n.ends_with(".html")).collect();
        on_disk.sort();
        let mut listed: Vec<String> = SOURCES.iter().map(|s| if s.path == "/" { "index.html".to_string() } else { s.path.rsplit('/').next().unwrap().to_string() }).collect();
        listed.sort();
        assert_eq!(on_disk, listed, "a file in web/ that is not in SOURCES would 404 in the binary");
    }

    #[test]
    fn the_shell_loads_exactly_the_listed_files_in_the_listed_order() {
        let shell = SOURCES[0].text;
        let attr = |prefix: &str| -> Vec<String> { shell.split(prefix).skip(1).filter_map(|r| r.split('"').next().map(String::from)).collect() };
        let scripts = attr("<script src=\"");
        let listed: Vec<String> = SOURCES.iter().filter(|s| s.content_type == JS).map(|s| s.path.to_string()).collect();
        assert_eq!(scripts, listed);
        assert_eq!(scripts.first().map(String::as_str), Some("/js/core.js"), "core.js loads first");
        assert_eq!(attr("<link rel=\"stylesheet\" href=\""), ["/app.css"]);
    }

    #[test]
    fn the_icon_href_stays_inside_its_attribute() {
        // A data: URI in an attribute may not contain a raw double quote or angle bracket: the first quote
        // ends the attribute and the rest of the SVG is parsed as page markup, leaving stray text on screen.
        let page = SOURCES[0].text;
        let at = page.find("rel=\"icon\"").expect("icon link");
        let after_href = page[at..].split_once("href=\"").expect("href attribute").1;
        let value = after_href.split('"').next().unwrap();
        assert!(value.starts_with("data:image/svg+xml,"), "value: {value}");
        assert!(!value.contains('<') && !value.contains('>'), "raw angle bracket in href: {value}");
        assert!(value.ends_with("%3C/svg%3E"), "href was cut short: {value}");
        assert!(after_href[value.len()..].starts_with("\">"), "attribute does not close the tag");
    }

    /// One request over a real socket to a server whose only handler is `reply`, the function
    /// `serve_on` calls, without its sampler and history threads (they write under the real home folder).
    fn fetch(host: &str, site: Option<&str>, path: &str) -> (u16, String, String) {
        use std::io::{Read, Write};
        let (server, port) = crate::server::bind_free().expect("bind");
        let hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
        let all = files(SECRET);
        let worker = Arc::clone(&server);
        let handle = std::thread::spawn(move || {
            let req = worker.incoming_requests().next().unwrap();
            let hdr = |n: &'static str| req.headers().iter().find(|h| h.field.equiv(n)).map(|h| h.value.to_string());
            let res = reply(&all, &hosts, hdr("Host").as_deref(), hdr("Sec-Fetch-Site").as_deref(), req.url());
            let _ = req.respond(res);
        });
        let host = host.replace("PORT", &port.to_string());
        let site_line = site.map(|s| format!("Sec-Fetch-Site: {s}\r\n")).unwrap_or_default();
        let mut conn = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(conn, "GET {path} HTTP/1.1\r\nHost: {host}\r\n{site_line}Connection: close\r\n\r\n").unwrap();
        let mut raw = String::new();
        conn.read_to_string(&mut raw).unwrap();
        handle.join().unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").unwrap();
        (head.split(' ').nth(1).unwrap().parse().unwrap(), head.to_ascii_lowercase(), body.to_string())
    }

    #[test]
    fn each_web_file_is_served_with_its_type_no_store_and_nosniff() {
        for s in SOURCES {
            let (status, head, body) = fetch("127.0.0.1:PORT", None, s.path);
            assert_eq!(status, 200, "{}", s.path);
            assert!(head.contains(&format!("content-type: {}", s.content_type)), "{}: {head}", s.path);
            assert!(head.contains("cache-control: no-store"), "{}: {head}", s.path);
            assert!(head.contains("x-content-type-options: nosniff"), "{}: {head}", s.path);
            assert!(!body.is_empty(), "{}", s.path);
            if s.path != "/" { assert!(!body.contains(SECRET), "{} must not carry the token", s.path); }
        }
        let shell = fetch("127.0.0.1:PORT", None, "/").2;
        assert_eq!(shell.matches(SECRET).count(), 1);
        assert!(shell.contains(&format!("<meta name=\"ml-token\" content=\"{SECRET}\">")));
    }

    #[test]
    fn unknown_paths_are_404_and_the_host_check_still_guards_every_file() {
        for path in ["/nope.js", "/js/", "/js/missing.js", "/index.html", "/../Cargo.toml", "/js/../Cargo.toml", "/web/core.js"] {
            assert_eq!(fetch("127.0.0.1:PORT", None, path).0, 404, "{path}");
        }
        for s in SOURCES {
            let (status, _, body) = fetch("evil.example:PORT", None, s.path);
            assert_eq!((status, body.as_str()), (403, "bad host"), "{} with a foreign Host", s.path);
        }
        assert_eq!(fetch("localhost:PORT", None, "/js/core.js").0, 200);
    }

    #[test]
    fn a_stylesheet_or_script_is_refused_to_another_site_and_allowed_to_this_one() {
        for s in SOURCES.iter().filter(|s| s.content_type != HTML) {
            for site in ["cross-site", "same-site"] {
                let (status, _, body) = fetch("127.0.0.1:PORT", Some(site), s.path);
                assert_eq!((status, body.as_str()), (403, "forbidden"), "{} for {site}", s.path);
            }
            for site in ["same-origin", "none"] { assert_eq!(fetch("127.0.0.1:PORT", Some(site), s.path).0, 200, "{} for {site}", s.path); }
        }
        // The address bar and a link from another site may open the page itself; its token cannot be read cross-origin.
        assert_eq!(fetch("127.0.0.1:PORT", Some("cross-site"), "/").0, 200);
    }
}
