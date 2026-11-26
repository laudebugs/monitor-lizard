//! Startup items: launch agents/daemons and cron. Read-only. A new item appearing after the first scan
//! is the classic sign of something installing itself, so we remember first-seen times, and a
//! fingerprint of each item so an edit is noticed too.
use crate::{baseline::{self, Kind}, collect, history::{self, History, Observed, PersistRow, LEARNING_SECS}, model::Signing};
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const NEW_WINDOW_SECS: u64 = 7 * 24 * 3600;
/// A plist larger than this is not read into memory; real ones are a few hundred bytes.
const MAX_PLIST_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, Default)]
pub struct Item {
    pub path: String, pub scope: &'static str, pub kind: &'static str, pub label: String, pub program: String,
    pub run_at_load: bool, pub keep_alive: bool, pub signing: Option<Signing>, pub first_seen: u64, pub is_new: bool,
    /// The user marked this startup item as expected; it is then never `is_new`.
    pub expected: bool,
    /// `StartInterval` in seconds, when the plist has one.
    pub start_interval: Option<u64>,
    /// When it runs, in words: "at load, kept alive, every 3600s", a cron expression, or "on demand".
    pub schedule: String,
    /// Hash of the plist bytes and the program's code-signing identity. Changes when either does.
    pub fingerprint: String,
    /// Set when the item is no longer found; the item stays listed with its last known facts.
    pub removed_at: Option<u64>,
    /// Why the plist could not be read; the item is listed anyway.
    pub unreadable: Option<String>,
    /// The program path is absolute and nothing exists there.
    pub program_missing: bool,
    /// Hash of the plist bytes alone: the part of the fingerprint known before signing is read.
    #[serde(skip)]
    content_hash: String,
}

/// What `take` returns and `/api/persistence` serves.
#[derive(Serialize, Debug, Default)]
pub struct Scan { pub items: Vec<Item>, pub changes: Vec<ChangeEntry> }

/// One item that was added, changed, restored or removed since the baseline date.
#[derive(Serialize, Debug, PartialEq)]
pub struct ChangeEntry { pub path: String, pub label: String, pub kind: &'static str, pub at: u64 }

extern "C" { fn CC_SHA256(data: *const u8, len: u32, md: *mut u8) -> *mut u8; }

/// Lowercase hex SHA-256. Uses the system's CommonCrypto so no dependency is needed. Input over 4 GiB
/// is not expected (plists are capped far below that).
fn sha256_hex(data: &[u8]) -> String {
    let mut md = [0u8; 32];
    // SAFETY: `data` is valid for `len` bytes and `md` has the 32 bytes CC_SHA256 writes.
    unsafe { CC_SHA256(data.as_ptr(), data.len() as u32, md.as_mut_ptr()); }
    md.iter().map(|b| format!("{b:02x}")).collect()
}

/// Names the program's code-signing identity for the fingerprint. Nothing in it changes between
/// scans unless the program or its signature did: no timestamps, no process ids.
fn identity(program: &str, program_missing: bool, signing: Option<&Signing>) -> String {
    match (program.is_empty(), program_missing, signing) {
        (true, _, _) => "no program".into(),
        (_, true, _) => "program missing".into(),
        (_, _, None) => "not checked".into(),
        (_, _, Some(s)) => format!("signed={} team={} authority={} apple={}", s.signed, s.team.as_deref().unwrap_or(""), s.authority.as_deref().unwrap_or(""), s.apple),
    }
}

fn fingerprint(content_hash: &str, program: &str, identity: &str) -> String {
    sha256_hex(format!("{content_hash}\0{program}\0{identity}").as_bytes())
}

/// `codesign` results by program path and modified time, so a scan every minute does not run
/// `codesign` for programs that did not change. A replaced program has a new time and is read again.
type SigningCache = Mutex<HashMap<String, (Option<SystemTime>, Signing)>>;

fn signing_cached_with(cache: &SigningCache, path: &str, modified: Option<SystemTime>, compute: impl FnOnce() -> Signing) -> Signing {
    if let Some((m, s)) = cache.lock().unwrap().get(path) { if *m == modified { return s.clone(); } }
    let s = compute();
    cache.lock().unwrap().insert(path.into(), (modified, s.clone()));
    s
}

fn signing_cached(path: &str) -> Signing {
    static CACHE: OnceLock<SigningCache> = OnceLock::new();
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    signing_cached_with(CACHE.get_or_init(Default::default), path, modified, || collect::signing_uncached(path))
}

fn schedule_text(run_at_load: bool, keep_alive: bool, interval: Option<u64>) -> String {
    let mut parts = Vec::new();
    if run_at_load { parts.push("at load".to_string()); }
    if keep_alive { parts.push("kept alive".into()); }
    if let Some(n) = interval { parts.push(format!("every {n}s")); }
    if parts.is_empty() { "on demand".into() } else { parts.join(", ") }
}

pub fn item_from_json(path: &str, scope: &'static str, kind: &'static str, v: &serde_json::Value) -> Item {
    let program = v["Program"].as_str().or_else(|| v["ProgramArguments"][0].as_str()).unwrap_or("").to_string();
    let keep_alive = match &v["KeepAlive"] { serde_json::Value::Bool(b) => *b, serde_json::Value::Object(_) => true, _ => false };
    let run_at_load = v["RunAtLoad"].as_bool().unwrap_or(false);
    let start_interval = v["StartInterval"].as_u64();
    Item {
        path: path.into(), scope, kind, label: v["Label"].as_str().unwrap_or(path.rsplit('/').next().unwrap_or(path)).into(),
        program, run_at_load, keep_alive, start_interval, schedule: schedule_text(run_at_load, keep_alive, start_interval), ..Default::default()
    }
}

/// An item whose plist could not be read or parsed: still listed, with the reason.
fn unreadable_item(path: &str, scope: &'static str, kind: &'static str, reason: String) -> Item {
    let stem = path.rsplit('/').next().unwrap_or(path);
    Item { path: path.into(), scope, kind, label: stem.strip_suffix(".plist").unwrap_or(stem).into(), schedule: "unknown".into(),
        content_hash: sha256_hex(reason.as_bytes()), unreadable: Some(reason), ..Default::default() }
}

/// One crontab line -> item (None for comments, blanks and env assignments).
pub fn item_from_cron(line: &str) -> Option<Item> {
    let l = line.trim();
    if l.is_empty() || l.starts_with('#') || (l.contains('=') && !l.contains(' ')) { return None; }
    let cmd = if l.starts_with('@') { l.split_once(char::is_whitespace)?.1.trim().to_string() }
              else { l.split_whitespace().skip(5).collect::<Vec<_>>().join(" ") };
    if cmd.is_empty() { return None; }
    let schedule = if l.starts_with('@') { l.split_whitespace().next().unwrap_or("").to_string() } else { l.split_whitespace().take(5).collect::<Vec<_>>().join(" ") };
    Some(Item { path: format!("crontab:{l}"), scope: "user", kind: "cron", label: cmd.chars().take(80).collect(), program: first_word(&cmd),
        run_at_load: l.starts_with("@reboot"), schedule, content_hash: sha256_hex(l.as_bytes()), ..Default::default() })
}

/// The program of a cron command: a quoted path may contain spaces, otherwise the first word.
fn first_word(cmd: &str) -> String {
    let c = cmd.trim_start();
    if let Some(q) = c.chars().next().filter(|q| *q == '"' || *q == '\'') {
        if let Some(end) = c[1..].find(q) { return c[1..1 + end].to_string(); }
    }
    c.split_whitespace().next().unwrap_or("").to_string()
}

/// Items from the output of `crontab -l`; lines that are not jobs ("no crontab for ...") yield nothing.
pub fn parse_crontab(out: &str) -> Vec<Item> { out.lines().filter_map(item_from_cron).collect() }

/// The first line of plutil's complaint without the file path, which the item already shows.
fn plutil_reason(stderr: &[u8], path: &str) -> String {
    let text = String::from_utf8_lossy(stderr);
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("not a valid property list");
    line.replace(&format!("{path}: "), "").replace(path, "the file")
}

fn plist_item(path: &str, scope: &'static str, kind: &'static str) -> Item {
    let bytes = match std::fs::metadata(path) {
        Ok(m) if m.len() > MAX_PLIST_BYTES => return unreadable_item(path, scope, kind, format!("file is larger than {MAX_PLIST_BYTES} bytes")),
        _ => match std::fs::read(path) { Ok(b) => b, Err(e) => return unreadable_item(path, scope, kind, format!("cannot read the file: {e}")) },
    };
    let out = match Command::new("plutil").args(["-convert", "json", "-o", "-", path]).output() {
        Ok(o) => o,
        Err(e) => return unreadable_item(path, scope, kind, format!("cannot run plutil: {e}")),
    };
    match serde_json::from_slice::<serde_json::Value>(&out.stdout).ok().filter(|_| out.status.success()) {
        Some(v) => Item { content_hash: sha256_hex(&bytes), ..item_from_json(path, scope, kind, &v) },
        // The bytes are part of the fingerprint even when unparsable, so editing the broken file still shows.
        None => Item { content_hash: sha256_hex(&bytes), ..unreadable_item(path, scope, kind, plutil_reason(&out.stderr, path)) },
    }
}

/// Reads every plist in `dir`. False when the folder could not be listed, so the caller does not
/// treat its items as removed.
fn scan_dir(dir: &str, scope: &'static str, kind: &'static str, out: &mut Vec<Item>) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else { return false };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("plist") { continue; }
        out.push(plist_item(&p.to_string_lossy(), scope, kind));
    }
    true
}

/// An item is new if it first appeared after the learning period that follows the very first scan,
/// and recently enough to still be worth a look.
pub fn is_new(first_seen: u64, earliest: u64, now: u64) -> bool {
    first_seen > earliest + LEARNING_SECS && now.saturating_sub(first_seen) < NEW_WINDOW_SECS
}

/// Kind and scope of an item that is gone, read from where it used to live.
fn kind_and_scope(path: &str, home: &str) -> (&'static str, &'static str) {
    if path.starts_with("crontab:") { ("cron", "user") }
    else if path.contains("/LaunchDaemons/") { ("LaunchDaemon", "system") }
    else if !home.is_empty() && path.starts_with(&format!("{home}/")) { ("LaunchAgent", "user") }
    else { ("LaunchAgent", "system") }
}

/// A listing entry for an item that is no longer found, built from what the database kept.
fn removed_item(row: &PersistRow, home: &str) -> Item {
    if let Some(mut it) = row.path.strip_prefix("crontab:").and_then(item_from_cron) {
        it.first_seen = row.first_seen; it.removed_at = row.removed_at; it.fingerprint = row.fingerprint.clone().unwrap_or_default();
        return it;
    }
    let (kind, scope) = kind_and_scope(&row.path, home);
    let stem = row.path.rsplit('/').next().unwrap_or(&row.path);
    Item { path: row.path.clone(), scope, kind, label: stem.strip_suffix(".plist").unwrap_or(stem).into(), schedule: row.schedule.clone(),
        first_seen: row.first_seen, removed_at: row.removed_at, fingerprint: row.fingerprint.clone().unwrap_or_default(), ..Default::default() }
}

pub fn take() -> Scan {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut items = Vec::new();
    let mut scanned = Vec::new();
    for (dir, scope, kind) in [(format!("{home}/Library/LaunchAgents"), "user", "LaunchAgent"), ("/Library/LaunchAgents".into(), "system", "LaunchAgent"), ("/Library/LaunchDaemons".into(), "system", "LaunchDaemon")] {
        if scan_dir(&dir, scope, kind, &mut items) { scanned.push(format!("{dir}/")); }
    }
    // `crontab -l` exits non-zero when the user has no crontab; that is an answer, not a failure.
    if let Ok(o) = Command::new("crontab").arg("-l").output() {
        if o.status.success() || String::from_utf8_lossy(&o.stderr).contains("no crontab") {
            scanned.push("crontab:".into());
            items.extend(parse_crontab(&String::from_utf8_lossy(&o.stdout)));
        }
    }
    std::thread::scope(|s| {
        for it in items.iter_mut().filter(|i| i.program.starts_with('/')) {
            it.program_missing = !std::path::Path::new(&it.program).exists();
            if !it.program_missing { s.spawn(move || it.signing = Some(signing_cached(&it.program))); }
        }
    });
    for it in &mut items { it.fingerprint = fingerprint(&it.content_hash, &it.program, &identity(&it.program, it.program_missing, it.signing.as_ref())); }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let base = baseline::load();
    let recorded = History::open().and_then(|mut h| {
        let seen: Vec<Observed> = items.iter().map(|i| Observed { path: &i.path, fingerprint: &i.fingerprint, schedule: &i.schedule }).collect();
        h.record_persistence(&seen, &scanned, now)
    });
    let mut changes = Vec::new();
    match recorded {
        Ok(rows) => {
            let earliest = rows.iter().map(|r| r.first_seen).min().unwrap_or(now);
            let first: HashMap<&str, u64> = rows.iter().map(|r| (r.path.as_str(), r.first_seen)).collect();
            for it in &mut items {
                it.first_seen = first.get(it.path.as_str()).copied().unwrap_or(now);
                it.expected = base.has(Kind::Startup, &it.path);
                it.is_new = is_new(it.first_seen, earliest, now) && !it.expected;
            }
            let present: std::collections::HashSet<String> = items.iter().map(|i| i.path.clone()).collect();
            items.extend(rows.iter().filter(|r| r.removed_at.is_some() && !present.contains(&r.path)).map(|r| {
                let mut it = removed_item(r, &home);
                it.expected = base.has(Kind::Startup, &it.path);
                it
            }));
            let label_of: HashMap<&str, &str> = items.iter().map(|i| (i.path.as_str(), i.label.as_str())).collect();
            // A marked item has been looked at, so it leaves the list like it leaves "new".
            changes = history::persistence_changes(&rows).into_iter().filter(|c| !base.has(Kind::Startup, &c.path))
                .map(|c| ChangeEntry { label: label_of.get(c.path.as_str()).copied().unwrap_or(&c.path).to_string(), path: c.path, kind: c.kind, at: c.at }).collect();
        }
        // Fail loudly: without history nothing can be called new, and silence would look like "all clear".
        Err(e) => {
            eprintln!("monitor-lizard: could not record startup items, so none can be marked new: {e}");
            for it in &mut items { it.first_seen = now; it.expected = base.has(Kind::Startup, &it.path); }
        }
    }
    items.sort_by(|a, b| a.removed_at.is_some().cmp(&b.removed_at.is_some()).then(b.is_new.cmp(&a.is_new)).then(a.scope.cmp(b.scope)).then(a.label.cmp(&b.label)));
    Scan { items, changes }
}

/// Sets `expected` on one item of a cached scan and clears `is_new` when it becomes expected. A marked
/// item also leaves the change list. Anything that cannot be parsed is returned as it was.
pub fn patch_expected(json: &str, path: &str, expected: bool) -> String {
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(json) else { return json.into() };
    for item in v["items"].as_array_mut().into_iter().flatten().filter(|i| i["path"] == path) {
        item["expected"] = expected.into();
        if expected { item["is_new"] = false.into(); }
    }
    if expected { if let Some(c) = v["changes"].as_array_mut() { c.retain(|c| c["path"] != path); } }
    serde_json::to_string(&v).unwrap_or_else(|_| json.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patching_the_cached_list_marks_only_the_named_item() {
        let json = r#"{"items":[{"path":"/a","is_new":true,"expected":false},{"path":"/b","is_new":true,"expected":false}],"changes":[{"path":"/a","kind":"added"},{"path":"/b","kind":"added"}]}"#;
        let v: serde_json::Value = serde_json::from_str(&patch_expected(json, "/a", true)).unwrap();
        let (a, b) = (&v["items"][0], &v["items"][1]);
        assert_eq!((a["expected"].as_bool(), a["is_new"].as_bool()), (Some(true), Some(false)));
        assert_eq!((b["expected"].as_bool(), b["is_new"].as_bool()), (Some(false), Some(true)));
        assert_eq!(v["changes"].as_array().unwrap().len(), 1, "a marked item leaves the change list");
        assert_eq!(v["changes"][0]["path"], "/b");
        assert_eq!(patch_expected("not json", "/a", true), "not json");
    }
    #[test]
    fn plist_json() {
        let v: serde_json::Value = serde_json::from_str(r#"{"Label":"com.x.agent","ProgramArguments":["/opt/x/bin/agent","--run"],"RunAtLoad":true,"KeepAlive":{"SuccessfulExit":false}}"#).unwrap();
        let it = item_from_json("/Library/LaunchAgents/com.x.agent.plist", "system", "LaunchAgent", &v);
        assert_eq!((it.label.as_str(), it.program.as_str(), it.run_at_load, it.keep_alive), ("com.x.agent", "/opt/x/bin/agent", true, true));
    }
    #[test]
    fn newness_follows_the_learning_period_and_window() {
        assert!(!is_new(1000, 1000, 1000), "first scan: nothing is new");
        assert!(!is_new(1050, 1000, 1100), "inside the learning period");
        assert!(is_new(1200, 1000, 1300), "appeared after learning");
        assert!(!is_new(1200, 1000, 1200 + NEW_WINDOW_SECS), "too old to flag");
    }
    #[test]
    fn cron_program_may_be_quoted_and_contain_spaces() {
        let it = item_from_cron(r#"0 * * * * "/Applications/Foo Bar.app/run" --x"#).unwrap();
        assert_eq!(it.program, "/Applications/Foo Bar.app/run");
        assert_eq!(item_from_cron("0 * * * * '/opt/a b/c'").unwrap().program, "/opt/a b/c");
        assert_eq!(item_from_cron("0 * * * * /usr/bin/true --x").unwrap().program, "/usr/bin/true");
    }
    #[test]
    fn crontab_output_without_jobs_gives_no_items() {
        assert!(parse_crontab("").is_empty());
        assert!(parse_crontab("no crontab for someone\n").is_empty());
        assert_eq!(parse_crontab("# m h dom mon dow\nMAILTO=x\n*/5 * * * * /bin/echo hi\n").len(), 1);
    }
    #[test]
    fn scan_lists_unreadable_plists_with_a_reason_skips_other_files_and_keeps_spaces_in_programs() {
        let dir = std::env::temp_dir().join(format!("ow-scan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a plist").unwrap();
        std::fs::write(dir.join("garbage.plist"), "this is not a property list").unwrap();
        std::fs::write(dir.join("ok.plist"), r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>Label</key><string>com.test.ok</string><key>Program</key><string>/Applications/Foo Bar.app/Contents/MacOS/foo</string></dict></plist>"#).unwrap();
        let mut out = Vec::new();
        assert!(scan_dir(dir.to_str().unwrap(), "user", "LaunchAgent", &mut out));
        let _ = std::fs::remove_dir_all(&dir);
        out.sort_by_key(|i| i.unreadable.is_none());
        assert_eq!(out.len(), 2, "the valid and the broken plist are listed; the text file is not");
        assert_eq!(out[0].label, "garbage");
        let why = out[0].unreadable.as_deref().expect("the reason is kept");
        assert!(!why.is_empty() && !why.contains("garbage.plist"), "a reason without the path: {why}");
        assert_eq!((out[1].label.as_str(), out[1].program.as_str()), ("com.test.ok", "/Applications/Foo Bar.app/Contents/MacOS/foo"));
    }
    #[test]
    fn cron_lines() {
        assert!(item_from_cron("# comment").is_none());
        assert!(item_from_cron("PATH=/usr/bin").is_none());
        let it = item_from_cron("*/5 * * * * /usr/local/bin/backup --fast").unwrap();
        assert_eq!(it.program, "/usr/local/bin/backup");
        let r = item_from_cron("@reboot /opt/x/start").unwrap();
        assert!(r.run_at_load && r.program == "/opt/x/start");
    }
    fn plist(label: &str, program: &str, extra: &str) -> String {
        format!(r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>Label</key><string>{label}</string><key>Program</key><string>{program}</string>{extra}</dict></plist>"#)
    }
    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ml-pers-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn scan_one(dir: &std::path::Path) -> Vec<Item> {
        let mut out = Vec::new();
        scan_dir(dir.to_str().unwrap(), "user", "LaunchAgent", &mut out);
        out
    }
    fn print(it: &Item) -> String { fingerprint(&it.content_hash, &it.program, &identity(&it.program, false, it.signing.as_ref())) }

    #[test]
    fn schedule_keys_come_from_the_plist() {
        let v: serde_json::Value = serde_json::from_str(r#"{"Label":"x","Program":"/bin/x","RunAtLoad":true,"KeepAlive":true,"StartInterval":3600}"#).unwrap();
        let it = item_from_json("/p.plist", "user", "LaunchAgent", &v);
        assert_eq!((it.schedule.as_str(), it.start_interval), ("at load, kept alive, every 3600s", Some(3600)));
        let none: serde_json::Value = serde_json::from_str(r#"{"Label":"x"}"#).unwrap();
        assert_eq!(item_from_json("/p.plist", "user", "LaunchAgent", &none).schedule, "on demand");
        assert_eq!(item_from_cron("*/5 * * * * /bin/echo hi").unwrap().schedule, "*/5 * * * *");
        assert_eq!(item_from_cron("@reboot /opt/x/start").unwrap().schedule, "@reboot");
    }

    #[test]
    fn the_same_plist_gives_the_same_fingerprint_on_every_scan() {
        let d = scratch("stable");
        std::fs::write(d.join("a.plist"), plist("com.t.a", "/bin/ls", "")).unwrap();
        let (a, b) = (scan_one(&d), scan_one(&d));
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(print(&a[0]), print(&b[0]));
        assert_eq!(print(&a[0]).len(), 64);
    }

    #[test]
    fn editing_the_program_path_changes_the_fingerprint() {
        let d = scratch("edit");
        std::fs::write(d.join("a.plist"), plist("com.t.a", "/bin/ls", "")).unwrap();
        let before = print(&scan_one(&d)[0]);
        std::fs::write(d.join("a.plist"), plist("com.t.a", "/tmp/evil", "")).unwrap();
        let after = print(&scan_one(&d)[0]);
        let _ = std::fs::remove_dir_all(&d);
        assert_ne!(before, after);
    }

    #[test]
    fn a_new_signature_changes_the_fingerprint_with_the_plist_untouched() {
        let unsigned = Signing { signed: false, ..Default::default() };
        let signed = Signing { signed: true, team: Some("ABCDE12345".into()), authority: Some("Developer ID".into()), apple: false };
        let (h, p) = (sha256_hex(b"same plist"), "/opt/x/bin");
        assert_ne!(fingerprint(&h, p, &identity(p, false, Some(&unsigned))), fingerprint(&h, p, &identity(p, false, Some(&signed))));
        assert_ne!(identity(p, false, Some(&signed)), identity(p, true, None), "a missing program has its own identity");
    }

    #[test]
    fn sha256_matches_the_known_digest() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn a_missing_program_is_flagged_and_an_unreadable_plist_is_not_dropped() {
        let d = scratch("missing");
        std::fs::write(d.join("a.plist"), plist("com.t.a", "/nonexistent/prog", "")).unwrap();
        std::fs::write(d.join("b.plist"), "not a plist").unwrap();
        let mut items = scan_one(&d);
        let _ = std::fs::remove_dir_all(&d);
        items.sort_by_key(|i| i.unreadable.is_none());
        assert_eq!(items.len(), 2);
        assert!(items[0].unreadable.is_some() && items[0].program.is_empty());
        assert_eq!(identity("/nonexistent/prog", true, None), "program missing");
    }

    #[test]
    fn codesign_runs_once_per_path_and_modified_time() {
        let cache = SigningCache::default();
        let calls = std::cell::Cell::new(0);
        let read = |t: Option<SystemTime>| signing_cached_with(&cache, "/p", t, || { calls.set(calls.get() + 1); Signing::default() });
        let t1 = Some(UNIX_EPOCH + std::time::Duration::from_secs(10));
        read(t1); read(t1); read(t1);
        assert_eq!(calls.get(), 1, "an unchanged program is not read again");
        read(Some(UNIX_EPOCH + std::time::Duration::from_secs(11)));
        assert_eq!(calls.get(), 2, "a replaced program is");
    }

    #[test]
    fn a_removed_item_keeps_a_listing_built_from_its_row() {
        let row = PersistRow { path: "/Users/me/Library/LaunchAgents/com.x.plist".into(), first_seen: 5, schedule: "at load".into(), fingerprint: Some("f".into()), removed_at: Some(9), changed_at: None, change_kind: None };
        let it = removed_item(&row, "/Users/me");
        assert_eq!((it.label.as_str(), it.kind, it.scope, it.removed_at, it.schedule.as_str()), ("com.x", "LaunchAgent", "user", Some(9), "at load"));
        let cron = PersistRow { path: "crontab:*/5 * * * * /bin/echo hi".into(), ..row };
        assert_eq!(removed_item(&cron, "/Users/me").kind, "cron");
    }
}
