//! Startup items: launch agents/daemons and cron. Read-only. A new item appearing after the first scan
//! is the classic sign of something installing itself, so we remember first-seen times, and a
//! fingerprint of each item so an edit is noticed too.
use crate::{baseline::{self, Baseline, Kind}, collect, history::{self, ChangeKind, History, Observed, PersistRow, LEARNING_SECS}, model::Signing};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const NEW_WINDOW_SECS: u64 = 7 * 24 * 3600;
/// A plist larger than this is not read; real ones are a few hundred bytes.
const MAX_PLIST_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, Default)]
pub struct Item {
    pub path: String, pub scope: &'static str, pub kind: &'static str, pub label: String, pub program: String,
    pub run_at_load: bool, pub keep_alive: bool, pub signing: Option<Signing>, pub first_seen: u64,
    /// New since the baseline, or an item the user marked expected that has since changed.
    pub is_new: bool,
    /// The user marked this exact version of the item (same fingerprint) as expected.
    pub expected: bool,
    /// The user marked an earlier version as expected, and the item has changed since: it needs a look again.
    pub was_expected: bool,
    /// The key to mark this version as expected (or to unmark it, when `expected`). The server's
    /// baseline stores it, so the page never builds keys.
    pub mark_key: String,
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
    /// Why there is no signing result ("not checked", "program missing"); never read as signed.
    pub trust_note: Option<String>,
    /// Hash of the plist bytes alone: the part of the fingerprint known before signing is read.
    #[serde(skip)]
    content_hash: String,
}

/// What `take` returns and `/api/persistence` serves.
#[derive(Serialize, Debug, Default)]
pub struct Scan {
    pub items: Vec<Item>,
    /// Added, changed, restored or removed since the baseline: the last 30 days, at most 200, newest first.
    pub changes: Vec<ChangeEntry>,
    /// True when `changes` left entries out because of those limits.
    pub truncated: bool,
    /// When this scan started (seconds) and its number in this run; a later scan has a larger number.
    pub scanned_at: u64,
    pub scan_id: u64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ChangeEntry {
    pub path: String, pub label: String, pub kind: ChangeKind, pub at: u64,
    pub restored: bool, pub changed_since_added: bool, pub last_change_at: Option<u64>,
    /// The item had been marked expected before this change.
    pub was_expected: bool,
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
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

/// The reason an item has no signing result. A program that cannot be located is never shown as signed.
fn trust_note(program: &str, missing: bool) -> Option<String> {
    if program.is_empty() { None }
    else if missing { Some("program missing".into()) }
    else if !program.starts_with('/') { Some("not checked: the program path is relative".into()) }
    else { None }
}

/// The key a startup mark is stored under: the item and the exact version of it that was marked.
fn mark_key(path: &str, fingerprint: &str) -> String { format!("{path}#{fingerprint}") }

fn schedule_text(run_at_load: bool, keep_alive: bool, interval: Option<u64>) -> String {
    let mut parts = Vec::new();
    if run_at_load { parts.push("at load".to_string()); }
    if keep_alive { parts.push("kept alive".into()); }
    if let Some(n) = interval { parts.push(format!("every {n}s")); }
    if parts.is_empty() { "on demand".into() } else { parts.join(", ") }
}

fn label_from_path(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".plist").unwrap_or(name).into()
}

pub fn item_from_json(path: &str, scope: &'static str, kind: &'static str, v: &serde_json::Value) -> Item {
    let program = v["Program"].as_str().or_else(|| v["ProgramArguments"][0].as_str()).unwrap_or("").to_string();
    let keep_alive = match &v["KeepAlive"] { serde_json::Value::Bool(b) => *b, serde_json::Value::Object(_) => true, _ => false };
    let run_at_load = v["RunAtLoad"].as_bool().unwrap_or(false);
    let start_interval = v["StartInterval"].as_u64();
    Item {
        path: path.into(), scope, kind, label: v["Label"].as_str().map_or_else(|| label_from_path(path), String::from),
        program, run_at_load, keep_alive, start_interval, schedule: schedule_text(run_at_load, keep_alive, start_interval), ..Default::default()
    }
}

/// An item whose plist could not be read or parsed: still listed, with the reason.
fn unreadable_item(path: &str, scope: &'static str, kind: &'static str, reason: String) -> Item {
    Item { path: path.into(), scope, kind, label: label_from_path(path), schedule: "unknown".into(),
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

/// Reads one plist's bytes. The file is opened without blocking, so a pipe cannot hang the scan, and
/// the opened handle must be a regular file, so a link to a device such as /dev/zero is refused. A
/// link to an ordinary file is followed. At most the cap plus one byte is ever read.
fn read_plist(path: &str) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let f = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path).map_err(|e| format!("cannot read the file: {e}"))?;
    if !f.metadata().map_err(|e| format!("cannot read the file: {e}"))?.file_type().is_file() {
        return Err("not a regular file (it is a pipe, device or socket)".into());
    }
    let mut bytes = Vec::new();
    f.take(MAX_PLIST_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("cannot read the file: {e}"))?;
    if bytes.len() as u64 > MAX_PLIST_BYTES { return Err(format!("file is larger than {MAX_PLIST_BYTES} bytes")); }
    Ok(bytes)
}

/// Converts plist bytes to JSON with plutil, feeding the same bytes that were hashed, so the fields and the fingerprint cannot describe different versions of the file.
fn plist_to_json(bytes: &[u8]) -> Result<serde_json::Value, String> {
    let mut child = Command::new("plutil").args(["-convert", "json", "-o", "-", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| format!("cannot run plutil: {e}"))?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let data = bytes.to_vec();
    // A separate thread, so a large input cannot block on plutil's full output pipe.
    let writer = std::thread::spawn(move || { let _ = stdin.write_all(&data); });
    let out = child.wait_with_output().map_err(|e| format!("cannot run plutil: {e}"))?;
    let _ = writer.join();
    if !out.status.success() {
        let text = String::from_utf8_lossy(&out.stderr);
        let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("not a valid property list");
        return Err(line.strip_prefix("<stdin>: ").unwrap_or(line).to_string());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("plutil gave unreadable output: {e}"))
}

fn plist_item(path: &str, scope: &'static str, kind: &'static str) -> Item {
    let bytes = match read_plist(path) { Ok(b) => b, Err(reason) => return unreadable_item(path, scope, kind, reason) };
    let content_hash = sha256_hex(&bytes);
    // The bytes are part of the fingerprint even when unparsable, so editing the broken file still shows.
    match plist_to_json(&bytes) {
        Ok(v) => Item { content_hash, ..item_from_json(path, scope, kind, &v) },
        Err(reason) => Item { content_hash, ..unreadable_item(path, scope, kind, reason) },
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
    let fingerprint = row.fingerprint.clone().unwrap_or_default();
    if let Some(mut it) = row.path.strip_prefix("crontab:").and_then(item_from_cron) {
        it.first_seen = row.first_seen; it.removed_at = row.removed_at; it.fingerprint = fingerprint;
        return it;
    }
    let (kind, scope) = kind_and_scope(&row.path, home);
    Item { path: row.path.clone(), scope, kind, label: label_from_path(&row.path), schedule: row.schedule.clone(),
        first_seen: row.first_seen, removed_at: row.removed_at, fingerprint, ..Default::default() }
}

/// Where a scan looks. Tests point it at a scratch folder.
struct Roots { home: String, user_agents: String, system_agents: String, system_daemons: String, cron: bool }

impl Roots {
    fn real() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        Roots { user_agents: format!("{home}/Library/LaunchAgents"), system_agents: "/Library/LaunchAgents".into(), system_daemons: "/Library/LaunchDaemons".into(), cron: true, home }
    }
}

/// Paths that have at least one startup mark made at some version, so a changed item can say it was once expected.
fn marked_paths(base: &Baseline) -> HashSet<String> {
    base.entries().iter().filter_map(|e| baseline::parse_entry(e).1.rsplit_once('#').filter(|_| e.starts_with("startup:")).map(|(p, _)| p.to_string())).collect()
}

/// Sets the mark fields from the baseline. A mark names the version it was made on (`path#fingerprint`);
/// an old entry that is only the path (from before marks named a version) keeps matching any version.
fn apply_baseline(it: &mut Item, base: &Baseline, marked: &HashSet<String>) {
    let exact = mark_key(&it.path, &it.fingerprint);
    (it.expected, it.mark_key) = if base.has(Kind::Startup, &exact) { (true, exact) }
        else if base.has(Kind::Startup, &it.path) { (true, it.path.clone()) }
        else { (false, exact) };
    it.was_expected = !it.expected && marked.contains(&it.path);
}

fn scan_in(roots: &Roots, history: rusqlite::Result<History>, base: &Baseline, now: u64) -> Scan {
    let mut items = Vec::new();
    let mut scanned = Vec::new();
    for (dir, scope, kind) in [(&roots.user_agents, "user", "LaunchAgent"), (&roots.system_agents, "system", "LaunchAgent"), (&roots.system_daemons, "system", "LaunchDaemon")] {
        if scan_dir(dir, scope, kind, &mut items) { scanned.push(format!("{dir}/")); }
    }
    // `crontab -l` exits non-zero when the user has no crontab; that is an answer, not a failure.
    if roots.cron {
        if let Ok(o) = Command::new("crontab").arg("-l").output() {
            if o.status.success() || String::from_utf8_lossy(&o.stderr).contains("no crontab") {
                scanned.push("crontab:".into());
                items.extend(parse_crontab(&String::from_utf8_lossy(&o.stdout)));
            }
        }
    }
    std::thread::scope(|s| {
        for it in items.iter_mut().filter(|i| i.program.starts_with('/')) {
            it.program_missing = !std::path::Path::new(&it.program).exists();
            if !it.program_missing { s.spawn(move || it.signing = Some(collect::signing_of(&it.program))); }
        }
    });
    for it in &mut items {
        it.trust_note = trust_note(&it.program, it.program_missing);
        it.fingerprint = fingerprint(&it.content_hash, &it.program, &identity(&it.program, it.program_missing, it.signing.as_ref()));
    }
    let marked = marked_paths(base);
    for it in &mut items { apply_baseline(it, base, &marked); }
    let recorded = history.and_then(|mut h| {
        let seen: Vec<Observed> = items.iter().map(|i| Observed { path: &i.path, fingerprint: &i.fingerprint, schedule: &i.schedule }).collect();
        h.record_persistence(&seen, &scanned, now)
    });
    let (mut changes, mut truncated) = (Vec::new(), false);
    match recorded {
        Ok(rows) => {
            let earliest = rows.iter().map(|r| r.first_seen).min().unwrap_or(now);
            let first: HashMap<&str, u64> = rows.iter().map(|r| (r.path.as_str(), r.first_seen)).collect();
            for it in &mut items {
                it.first_seen = first.get(it.path.as_str()).copied().unwrap_or(now);
                it.is_new = (is_new(it.first_seen, earliest, now) && !it.expected) || it.was_expected;
            }
            let present: HashSet<String> = items.iter().map(|i| i.path.clone()).collect();
            let gone: Vec<Item> = rows.iter().filter(|r| r.removed_at.is_some() && !present.contains(&r.path)).map(|r| {
                let mut it = removed_item(r, &roots.home);
                apply_baseline(&mut it, base, &marked);
                it
            }).collect();
            items.extend(gone);
            let by_path: HashMap<&str, &Item> = items.iter().map(|i| (i.path.as_str(), i)).collect();
            let (list, cut) = history::persistence_changes(&rows, now);
            truncated = cut;
            // An item marked at its current version has been looked at, so it leaves the list; a changed one stays.
            changes = list.into_iter().filter(|c| !by_path.get(c.path.as_str()).is_some_and(|i| i.expected)).map(|c| {
                let item = by_path.get(c.path.as_str());
                let was_expected = item.is_some_and(|i| i.was_expected);
                // An edit after a mark is what matters, even on an item added since the baseline.
                let (kind, at) = match (was_expected, c.last_change_at) { (true, Some(t)) => (ChangeKind::Changed, t), _ => (c.kind, c.at) };
                ChangeEntry { label: item.map_or_else(|| c.path.clone(), |i| i.label.clone()), path: c.path, kind, at, restored: c.restored,
                    changed_since_added: c.changed_since_added, last_change_at: c.last_change_at, was_expected }
            }).collect();
        }
        // Fail loudly: without history nothing can be called new, and silence would look like "all clear".
        Err(e) => {
            eprintln!("monitor-lizard: could not record startup items, so none can be marked new: {e}");
            for it in &mut items { it.first_seen = now; it.is_new = it.was_expected; }
        }
    }
    items.sort_by(|a, b| a.removed_at.is_some().cmp(&b.removed_at.is_some()).then(b.is_new.cmp(&a.is_new)).then(a.scope.cmp(b.scope)).then(a.label.cmp(&b.label)));
    Scan { items, changes, truncated, scanned_at: now, scan_id: 0 }
}

fn now_secs() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

/// Held for a whole scan and the cache update after it, and counts scans. Two scans (the periodic
/// one and one started by a mark) must not interleave: a stale one committing last could mark an
/// item removed and then restored, or put older data in the cache.
static SCAN_GATE: Mutex<u64> = Mutex::new(0);

fn run_serialized(scan: impl FnOnce() -> Scan, cache: Option<&Mutex<Option<String>>>) -> Scan {
    let mut last = SCAN_GATE.lock().unwrap_or_else(|e| e.into_inner());
    *last += 1;
    let mut s = scan();
    s.scan_id = *last;
    if let Some(c) = cache { *c.lock().unwrap() = Some(serde_json::to_string(&s).unwrap_or_default()); }
    s
}

fn scan_real() -> Scan { scan_in(&Roots::real(), History::open(), &baseline::load(), now_secs()) }

pub fn take() -> Scan { run_serialized(scan_real, None) }

/// Scans and stores the result as JSON in the server's cache, in one step that no other scan can interleave with.
pub fn refresh_cache(cache: &Mutex<Option<String>>) { run_serialized(scan_real, Some(cache)); }

/// Sets `expected` on the item with this mark key in a cached scan, and clears `is_new` when it becomes
/// expected. A marked item also leaves the change list. Anything unparsable is returned as it was.
pub fn patch_expected(json: &str, key: &str, expected: bool) -> String {
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(json) else { return json.into() };
    let mut paths = Vec::new();
    for item in v["items"].as_array_mut().into_iter().flatten().filter(|i| i["mark_key"] == key) {
        item["expected"] = expected.into();
        if expected { item["is_new"] = false.into(); item["was_expected"] = false.into(); }
        paths.push(item["path"].clone());
    }
    if expected { if let Some(c) = v["changes"].as_array_mut() { c.retain(|c| !paths.contains(&c["path"])); } }
    serde_json::to_string(&v).unwrap_or_else(|_| json.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patching_the_cached_list_marks_only_the_named_item() {
        let json = r#"{"items":[{"path":"/a","mark_key":"/a#f1","is_new":true,"expected":false},{"path":"/b","mark_key":"/b#f2","is_new":true,"expected":false}],"changes":[{"path":"/a","kind":"added"},{"path":"/b","kind":"added"}]}"#;
        let v: serde_json::Value = serde_json::from_str(&patch_expected(json, "/a#f1", true)).unwrap();
        let (a, b) = (&v["items"][0], &v["items"][1]);
        assert_eq!((a["expected"].as_bool(), a["is_new"].as_bool()), (Some(true), Some(false)));
        assert_eq!((b["expected"].as_bool(), b["is_new"].as_bool()), (Some(false), Some(true)));
        assert_eq!(v["changes"].as_array().unwrap().len(), 1, "a marked item leaves the change list");
        assert_eq!(v["changes"][0]["path"], "/b");
        assert_eq!(patch_expected("not json", "/a#f1", true), "not json");
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
    fn a_removed_item_keeps_a_listing_built_from_its_row() {
        let row = PersistRow { path: "/Users/me/Library/LaunchAgents/com.x.plist".into(), first_seen: 5, schedule: "at load".into(), fingerprint: Some("f".into()), removed_at: Some(9), changed_at: None, restored_at: None };
        let it = removed_item(&row, "/Users/me");
        assert_eq!((it.label.as_str(), it.kind, it.scope, it.removed_at, it.schedule.as_str()), ("com.x", "LaunchAgent", "user", Some(9), "at load"));
        let cron = PersistRow { path: "crontab:*/5 * * * * /bin/echo hi".into(), ..row };
        assert_eq!(removed_item(&cron, "/Users/me").kind, "cron");
    }

    // ----- marks are tied to the version that was marked -----

    fn roots(dir: &std::path::Path) -> Roots {
        Roots { home: dir.to_string_lossy().into(), user_agents: dir.join("LaunchAgents").to_string_lossy().into(),
            system_agents: "/nonexistent-ml/agents".into(), system_daemons: "/nonexistent-ml/daemons".into(), cron: false }
    }
    fn scan_at(r: &Roots, db: &std::path::Path, base: &Baseline, now: u64) -> Scan { scan_in(r, History::at(db), base, now) }
    fn find<'a>(s: &'a Scan, label: &str) -> &'a Item { s.items.iter().find(|i| i.label == label).unwrap_or_else(|| panic!("no {label}")) }
    fn write_plist(r: &Roots, name: &str, program: &str) -> String {
        std::fs::create_dir_all(&r.user_agents).unwrap();
        let path = format!("{}/{name}.plist", r.user_agents);
        std::fs::write(&path, plist(&format!("com.t.{name}"), program, "<key>RunAtLoad</key><true/>")).unwrap();
        path
    }
    fn unsigned_program(dir: &std::path::Path) -> String {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("unsigned.sh");
        std::fs::write(&p, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_string_lossy().into()
    }

    #[test]
    fn a_marked_item_whose_program_is_replaced_by_an_unsigned_one_is_flagged_again() {
        let d = scratch("marked-changed");
        let (r, db) = (roots(&d), d.join("h.db"));
        let path = write_plist(&r, "a", "/bin/ls");
        let none = Baseline::default();
        let marked_key = find(&scan_at(&r, &db, &none, 1000), "com.t.a").mark_key.clone();
        assert!(marked_key.starts_with(&format!("{path}#")), "the key names the version");
        let base = Baseline::from_entries(vec![format!("startup:{marked_key}")]);
        let s = scan_at(&r, &db, &base, 2000);
        assert!(find(&s, "com.t.a").expected && s.changes.is_empty(), "the marked version is expected");
        write_plist(&r, "a", &unsigned_program(&d));
        let s = scan_at(&r, &db, &base, 3000);
        let it = find(&s, "com.t.a");
        let _ = std::fs::remove_dir_all(&d);
        assert!(!it.expected && it.was_expected && it.is_new, "needs attention again");
        assert_eq!(it.signing.as_ref().map(|g| g.signed), Some(false));
        assert_eq!(s.changes.len(), 1);
        assert_eq!((s.changes[0].kind, s.changes[0].was_expected, s.changes[0].at), (ChangeKind::Changed, true, 3000));
    }

    #[test]
    fn an_entry_without_a_fingerprint_matches_any_version() {
        let d = scratch("legacy");
        let (r, db) = (roots(&d), d.join("h.db"));
        let path = write_plist(&r, "a", "/bin/ls");
        let base = Baseline::from_entries(vec![format!("startup:{path}")]);
        scan_at(&r, &db, &base, 1000);
        write_plist(&r, "a", &unsigned_program(&d));
        let s = scan_at(&r, &db, &base, 3000);
        let _ = std::fs::remove_dir_all(&d);
        let it = find(&s, "com.t.a");
        assert!(it.expected && !it.was_expected);
        assert_eq!(it.mark_key, path, "unmarking removes the old entry");
    }

    // ----- the whole scan, end to end -----

    #[test]
    fn editing_a_plist_shows_under_changes_and_deleting_one_keeps_it_listed_as_removed() {
        let d = scratch("e2e");
        let (r, db, none) = (roots(&d), d.join("h.db"), Baseline::default());
        let path = write_plist(&r, "a", "/bin/ls");
        write_plist(&r, "b", "/bin/ls");
        let s = scan_at(&r, &db, &none, 1000);
        assert!(s.changes.is_empty(), "first run: everything is baseline");
        write_plist(&r, "a", "/nonexistent/evil");
        let s = scan_at(&r, &db, &none, 2000);
        assert_eq!(s.changes.len(), 1);
        assert_eq!((s.changes[0].path.as_str(), s.changes[0].kind), (path.as_str(), ChangeKind::Changed));
        assert!(find(&s, "com.t.a").program_missing);
        std::fs::remove_file(&path).unwrap();
        let s = scan_at(&r, &db, &none, 3000);
        let gone = s.items.iter().find(|i| i.path == path).expect("still listed");
        assert_eq!(gone.removed_at, Some(3000));
        assert!(s.changes.iter().any(|c| c.path == path && c.kind == ChangeKind::Removed));
        let s = scan_at(&r, &db, &none, 3100);
        assert_eq!(s.items.iter().find(|i| i.path == path).unwrap().removed_at, Some(3000), "the removal time is not rewritten");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_relative_program_path_is_never_shown_as_signed() {
        let d = scratch("relative");
        let (r, db) = (roots(&d), d.join("h.db"));
        write_plist(&r, "a", "bin/tool");
        let s = scan_at(&r, &db, &Baseline::default(), 1000);
        let _ = std::fs::remove_dir_all(&d);
        let it = find(&s, "com.t.a");
        assert!(it.signing.is_none() && !it.program_missing);
        assert_eq!(it.trust_note.as_deref(), Some("not checked: the program path is relative"));
        assert_eq!(identity("bin/tool", false, None), "not checked");
    }

    // ----- hostile files -----

    #[test]
    fn a_named_pipe_is_refused_without_blocking() {
        let d = scratch("fifo");
        let path = d.join("pipe.plist");
        let c = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        // SAFETY: `c` is a valid NUL-terminated path.
        if unsafe { libc::mkfifo(c.as_ptr(), 0o600) } != 0 { eprintln!("mkfifo unavailable; skipping"); return; }
        let it = plist_item(path.to_str().unwrap(), "user", "LaunchAgent");
        let _ = std::fs::remove_dir_all(&d);
        assert!(it.unreadable.as_deref().is_some_and(|w| w.contains("not a regular file")), "{:?}", it.unreadable);
    }

    #[test]
    fn a_file_over_the_cap_is_refused_with_a_reason_and_still_listed() {
        let d = scratch("big");
        let path = d.join("big.plist");
        std::fs::File::create(&path).unwrap().set_len(MAX_PLIST_BYTES + 1).unwrap();
        let it = plist_item(path.to_str().unwrap(), "user", "LaunchAgent");
        let _ = std::fs::remove_dir_all(&d);
        assert!(it.unreadable.as_deref().is_some_and(|w| w.contains("larger than")), "{:?}", it.unreadable);
        assert_eq!(it.label, "big");
    }

    #[test]
    fn a_link_to_a_device_is_refused_and_a_link_to_a_plist_is_followed() {
        let d = scratch("links");
        std::fs::write(d.join("real.xml"), plist("com.t.real", "/bin/ls", "")).unwrap();
        std::os::unix::fs::symlink(d.join("real.xml"), d.join("good.plist")).unwrap();
        // The link is only opened without reading: a character device reports "not regular" at once.
        std::os::unix::fs::symlink("/dev/null", d.join("dev.plist")).unwrap();
        let good = plist_item(d.join("good.plist").to_str().unwrap(), "user", "LaunchAgent");
        let dev = plist_item(d.join("dev.plist").to_str().unwrap(), "user", "LaunchAgent");
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!((good.label.as_str(), good.unreadable.is_none()), ("com.t.real", true));
        assert!(dev.unreadable.as_deref().is_some_and(|w| w.contains("not a regular file")));
    }

    #[test]
    fn the_parsed_fields_come_from_the_bytes_that_were_hashed() {
        let bytes = plist("com.t.same", "/bin/ls", "").into_bytes();
        let v = plist_to_json(&bytes).unwrap();
        assert_eq!(v["Label"], "com.t.same");
        let err = plist_to_json(b"nope").unwrap_err();
        assert!(!err.is_empty() && !err.contains("<stdin>"), "{err}");
    }

    // ----- scans never interleave -----

    #[test]
    fn two_threads_scanning_at_once_run_one_after_the_other_and_the_cache_ends_with_the_newer_scan() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let (busy, cache) = (AtomicBool::new(false), Mutex::new(None));
        let ids = Mutex::new(Vec::new());
        std::thread::scope(|s| for _ in 0..2 {
            s.spawn(|| {
                let out = run_serialized(|| {
                    assert!(!busy.swap(true, Ordering::SeqCst), "two scans overlapped");
                    std::thread::sleep(std::time::Duration::from_millis(40));
                    busy.store(false, Ordering::SeqCst);
                    Scan::default()
                }, Some(&cache));
                ids.lock().unwrap().push(out.scan_id);
            });
        });
        let ids = ids.into_inner().unwrap();
        assert_ne!(ids[0], ids[1]);
        let stored: serde_json::Value = serde_json::from_str(cache.lock().unwrap().as_deref().unwrap()).unwrap();
        assert_eq!(stored["scan_id"].as_u64(), ids.iter().copied().max());
    }

    #[test]
    fn concurrent_scans_of_a_stable_folder_never_record_a_removal_or_a_change() {
        let d = scratch("race");
        let (r, db) = (roots(&d), d.join("h.db"));
        write_plist(&r, "a", "/bin/ls");
        scan_at(&r, &db, &Baseline::default(), 1000);
        std::thread::scope(|s| for t in 0..2u64 {
            let (r, db) = (&r, &db);
            s.spawn(move || for n in 0..4 { run_serialized(|| scan_at(r, db, &Baseline::default(), 2000 + t * 10 + n), None); });
        });
        let rows = History::at(&db).unwrap().record_persistence(&[], &[], 9000).unwrap();
        let _ = std::fs::remove_dir_all(&d);
        assert!(rows.iter().all(|r| r.removed_at.is_none() && r.changed_at.is_none() && r.restored_at.is_none()), "{rows:?}");
    }
}
