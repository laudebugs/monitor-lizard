//! Startup items: launch agents/daemons and cron. Read-only. A new item appearing after the first scan
//! is the classic sign of something installing itself, so we remember first-seen times.
use crate::{collect, history::History, model::Signing};
use serde::Serialize;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// After this long since the very first scan, a newly appearing item counts as "new".
const LEARNING_SECS: u64 = 60;
const NEW_WINDOW_SECS: u64 = 7 * 24 * 3600;

#[derive(Serialize, Clone, Debug)]
pub struct Item {
    pub path: String, pub scope: &'static str, pub kind: &'static str, pub label: String, pub program: String,
    pub run_at_load: bool, pub keep_alive: bool, pub signing: Option<Signing>, pub first_seen: u64, pub is_new: bool,
}

pub fn item_from_json(path: &str, scope: &'static str, kind: &'static str, v: &serde_json::Value) -> Item {
    let program = v["Program"].as_str().or_else(|| v["ProgramArguments"][0].as_str()).unwrap_or("").to_string();
    let keep_alive = match &v["KeepAlive"] { serde_json::Value::Bool(b) => *b, serde_json::Value::Object(_) => true, _ => false };
    Item {
        path: path.into(), scope, kind, label: v["Label"].as_str().unwrap_or(path.rsplit('/').next().unwrap_or(path)).into(),
        program, run_at_load: v["RunAtLoad"].as_bool().unwrap_or(false), keep_alive, signing: None, first_seen: 0, is_new: false,
    }
}

/// One crontab line -> item (None for comments, blanks and env assignments).
pub fn item_from_cron(line: &str) -> Option<Item> {
    let l = line.trim();
    if l.is_empty() || l.starts_with('#') || (l.contains('=') && !l.contains(' ')) { return None; }
    let cmd = if l.starts_with('@') { l.split_once(char::is_whitespace)?.1.trim().to_string() }
              else { l.split_whitespace().skip(5).collect::<Vec<_>>().join(" ") };
    if cmd.is_empty() { return None; }
    Some(Item { path: format!("crontab:{l}"), scope: "user", kind: "cron", label: cmd.chars().take(80).collect(), program: cmd.split_whitespace().next().unwrap_or("").into(),
        run_at_load: l.starts_with("@reboot"), keep_alive: false, signing: None, first_seen: 0, is_new: false })
}

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
    if let Ok(o) = Command::new("crontab").arg("-l").output() {
        items.extend(String::from_utf8_lossy(&o.stdout).lines().filter_map(item_from_cron));
    }
    std::thread::scope(|s| {
        for it in items.iter_mut().filter(|i| i.program.starts_with('/') && std::path::Path::new(&i.program).exists()) {
            s.spawn(move || it.signing = Some(collect::signing_of(&it.program)));
        }
    });
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if let Ok(mut h) = History::open() {
        if let Ok((first, earliest)) = h.record_persistence(&items, now) {
            for it in &mut items {
                it.first_seen = first.get(&it.path).copied().unwrap_or(now);
                it.is_new = is_new(it.first_seen, earliest, now);
            }
        }
    }
    items.sort_by(|a, b| b.is_new.cmp(&a.is_new).then(a.scope.cmp(b.scope)).then(a.label.cmp(&b.label)));
    items
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn cron_lines() {
        assert!(item_from_cron("# comment").is_none());
        assert!(item_from_cron("PATH=/usr/bin").is_none());
        let it = item_from_cron("*/5 * * * * /usr/local/bin/backup --fast").unwrap();
        assert_eq!(it.program, "/usr/local/bin/backup");
        let r = item_from_cron("@reboot /opt/x/start").unwrap();
        assert!(r.run_at_load && r.program == "/opt/x/start");
    }
}
