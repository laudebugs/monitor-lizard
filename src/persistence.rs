//! Startup items: launch agents/daemons and cron. Read-only. A new item appearing after the first scan
//! is the classic sign of something installing itself, so we remember first-seen times.
use crate::{baseline::{self, Kind}, collect, history::{History, LEARNING_SECS}, model::Signing};
use serde::Serialize;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const NEW_WINDOW_SECS: u64 = 7 * 24 * 3600;

#[derive(Serialize, Clone, Debug)]
pub struct Item {
    pub path: String, pub scope: &'static str, pub kind: &'static str, pub label: String, pub program: String,
    pub run_at_load: bool, pub keep_alive: bool, pub signing: Option<Signing>, pub first_seen: u64, pub is_new: bool,
    /// The user marked this startup item as expected; it is then never `is_new`.
    pub expected: bool,
}

pub fn item_from_json(path: &str, scope: &'static str, kind: &'static str, v: &serde_json::Value) -> Item {
    let program = v["Program"].as_str().or_else(|| v["ProgramArguments"][0].as_str()).unwrap_or("").to_string();
    let keep_alive = match &v["KeepAlive"] { serde_json::Value::Bool(b) => *b, serde_json::Value::Object(_) => true, _ => false };
    Item {
        path: path.into(), scope, kind, label: v["Label"].as_str().unwrap_or(path.rsplit('/').next().unwrap_or(path)).into(),
        program, run_at_load: v["RunAtLoad"].as_bool().unwrap_or(false), keep_alive, signing: None, first_seen: 0, is_new: false, expected: false,
    }
}

/// One crontab line -> item (None for comments, blanks and env assignments).
pub fn item_from_cron(line: &str) -> Option<Item> {
    let l = line.trim();
    if l.is_empty() || l.starts_with('#') || (l.contains('=') && !l.contains(' ')) { return None; }
    let cmd = if l.starts_with('@') { l.split_once(char::is_whitespace)?.1.trim().to_string() }
              else { l.split_whitespace().skip(5).collect::<Vec<_>>().join(" ") };
    if cmd.is_empty() { return None; }
    Some(Item { path: format!("crontab:{l}"), scope: "user", kind: "cron", label: cmd.chars().take(80).collect(), program: first_word(&cmd),
        run_at_load: l.starts_with("@reboot"), keep_alive: false, signing: None, first_seen: 0, is_new: false, expected: false })
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

fn scan_dir(dir: &str, scope: &'static str, kind: &'static str, out: &mut Vec<Item>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("plist") { continue; }
        let path = p.to_string_lossy().to_string();
        let json = Command::new("plutil").args(["-convert", "json", "-o", "-", &path]).output();
        if let Some(v) = json.ok().and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok()) {
            out.push(item_from_json(&path, scope, kind, &v));
        }
    }
}

/// An item is new if it first appeared after the learning period that follows the very first scan,
/// and recently enough to still be worth a look.
pub fn is_new(first_seen: u64, earliest: u64, now: u64) -> bool {
    first_seen > earliest + LEARNING_SECS && now.saturating_sub(first_seen) < NEW_WINDOW_SECS
}

pub fn take() -> Vec<Item> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut items = Vec::new();
    scan_dir(&format!("{home}/Library/LaunchAgents"), "user", "LaunchAgent", &mut items);
    scan_dir("/Library/LaunchAgents", "system", "LaunchAgent", &mut items);
    scan_dir("/Library/LaunchDaemons", "system", "LaunchDaemon", &mut items);
    if let Some(o) = Command::new("crontab").arg("-l").output().ok().filter(|o| o.status.success()) {
        items.extend(parse_crontab(&String::from_utf8_lossy(&o.stdout)));
    }
    std::thread::scope(|s| {
        for it in items.iter_mut().filter(|i| i.program.starts_with('/') && std::path::Path::new(&i.program).exists()) {
            s.spawn(move || it.signing = Some(collect::signing_of(&it.program)));
        }
    });
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let base = baseline::load();
    let recorded = History::open().and_then(|mut h| {
        let paths: Vec<&str> = items.iter().map(|i| i.path.as_str()).collect();
        h.record_persistence(&paths, now)
    });
    match recorded {
        Ok((first, earliest)) => for it in &mut items {
            it.first_seen = first.get(&it.path).copied().unwrap_or(now);
            it.expected = base.has(Kind::Startup, &it.path);
            it.is_new = is_new(it.first_seen, earliest, now) && !it.expected;
        },
        // Fail loudly: without history nothing can be called new, and silence would look like "all clear".
        Err(e) => {
            eprintln!("monitor-lizard: could not record startup items, so none can be marked new: {e}");
            for it in &mut items { it.first_seen = now; it.expected = base.has(Kind::Startup, &it.path); }
        }
    }
    items.sort_by(|a, b| b.is_new.cmp(&a.is_new).then(a.scope.cmp(b.scope)).then(a.label.cmp(&b.label)));
    items
}

/// Sets `expected` on one item of a cached startup list and clears `is_new` when it becomes expected.
/// Anything that cannot be parsed is returned as it was.
pub fn patch_expected(json: &str, path: &str, expected: bool) -> String {
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(json) else { return json.into() };
    for item in v.as_array_mut().into_iter().flatten().filter(|i| i["path"] == path) {
        item["expected"] = expected.into();
        if expected { item["is_new"] = false.into(); }
    }
    serde_json::to_string(&v).unwrap_or_else(|_| json.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patching_the_cached_list_marks_only_the_named_item() {
        let json = r#"[{"path":"/a","is_new":true,"expected":false},{"path":"/b","is_new":true,"expected":false}]"#;
        let v: serde_json::Value = serde_json::from_str(&patch_expected(json, "/a", true)).unwrap();
        assert_eq!((v[0]["expected"].as_bool(), v[0]["is_new"].as_bool()), (Some(true), Some(false)));
        assert_eq!((v[1]["expected"].as_bool(), v[1]["is_new"].as_bool()), (Some(false), Some(true)));
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
    fn scan_skips_unreadable_and_non_plist_files_and_keeps_spaces_in_programs() {
        let dir = std::env::temp_dir().join(format!("ow-scan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a plist").unwrap();
        std::fs::write(dir.join("garbage.plist"), "this is not a property list").unwrap();
        std::fs::write(dir.join("ok.plist"), r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>Label</key><string>com.test.ok</string><key>Program</key><string>/Applications/Foo Bar.app/Contents/MacOS/foo</string></dict></plist>"#).unwrap();
        let mut out = Vec::new();
        scan_dir(dir.to_str().unwrap(), "user", "LaunchAgent", &mut out);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(out.len(), 1, "only the valid plist is listed");
        assert_eq!((out[0].label.as_str(), out[0].program.as_str()), ("com.test.ok", "/Applications/Foo Bar.app/Contents/MacOS/foo"));
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
}
