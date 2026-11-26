//! The baseline: what the user has said is expected on this Mac. Each entry is a typed string,
//! `app:<group key>`, `host:<group key>|<address>`, `startup:<plist path>` or `finding:<group key>:<rule>`,
//! kept as a sorted JSON list in `baseline.json`.
use std::collections::{BTreeSet, HashSet};
use std::sync::Mutex;
use std::{fs, path::{Path, PathBuf}};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind { App, Host, Startup, Finding }

impl Kind {
    const ALL: [Kind; 4] = [Kind::App, Kind::Host, Kind::Startup, Kind::Finding];
    pub fn name(self) -> &'static str {
        match self { Kind::App => "app", Kind::Host => "host", Kind::Startup => "startup", Kind::Finding => "finding" }
    }
    pub fn parse(name: &str) -> Option<Kind> { Kind::ALL.into_iter().find(|k| k.name() == name) }
}

/// Longest key accepted: far above any real path, and stops one request growing the file without bound.
const MAX_KEY_LEN: usize = 2048;

/// What `load` last had to tell the user, kept so the dashboard can show it after the file is gone.
static NOTICE: Mutex<Option<String>> = Mutex::new(None);

fn file() -> PathBuf { crate::paths::data_dir().join("baseline.json") }

pub fn typed(kind: Kind, key: &str) -> String { format!("{}:{key}", kind.name()) }

/// The key for one remote address of one app group.
pub fn host_key(group_key: &str, addr: &str) -> String { format!("{group_key}|{addr}") }

/// Splits `kind:key`. An entry with no known prefix is from before baselines were typed, when every
/// entry was an app group key. Group keys such as `dev:claude-code` or `apple:/usr/bin/x` contain a
/// colon but never start with one of the four prefixes, so they are not mistaken for typed entries.
pub fn parse_entry(entry: &str) -> (Kind, &str) {
    Kind::ALL.into_iter().find_map(|k| entry.strip_prefix(k.name()).and_then(|r| r.strip_prefix(':')).map(|key| (k, key))).unwrap_or((Kind::App, entry))
}

/// Rejects keys that cannot name a real item. Existence on this machine is not checked: marking
/// something that is not here yet is allowed.
pub fn validate(kind: Kind, key: &str) -> Result<(), String> {
    if key.is_empty() { return Err("key is empty".into()); }
    if key.len() > MAX_KEY_LEN { return Err("key is too long".into()); }
    if key.chars().any(char::is_control) { return Err("key contains a control character".into()); }
    if kind == Kind::Host && !key.split_once('|').is_some_and(|(app, addr)| !app.is_empty() && !addr.is_empty()) {
        return Err("a host key is <app key>|<address>".into());
    }
    Ok(())
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Baseline(BTreeSet<String>);

impl Baseline {
    pub fn has(&self, kind: Kind, key: &str) -> bool { self.0.contains(&typed(kind, key)) }
    /// Group keys of approved apps.
    pub fn apps(&self) -> HashSet<String> {
        self.0.iter().filter_map(|e| Some(parse_entry(e)).filter(|(k, _)| *k == Kind::App).map(|(_, key)| key.to_string())).collect()
    }
    pub fn entries(&self) -> Vec<String> { self.0.iter().cloned().collect() }
    pub fn from_entries(entries: Vec<String>) -> Self {
        Baseline(entries.into_iter().map(|e| { let (k, key) = parse_entry(&e); typed(k, key) }).collect())
    }
}

/// Reads the baseline. A missing file is an empty baseline. A file that cannot be parsed is moved
/// aside to `<file>.bad`, so the next write cannot destroy it, and an empty baseline is used.
fn load_from(path: &Path) -> (Baseline, Option<String>) {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Baseline::default(), None),
        Err(e) => return (Baseline::default(), Some(format!("Could not read {}: {e}. Nothing is marked as expected until it is readable.", path.display()))),
    };
    match serde_json::from_str::<Vec<String>>(&text) {
        Ok(v) => (Baseline::from_entries(v), None),
        Err(e) => {
            let bad = PathBuf::from(format!("{}.bad", path.display()));
            let moved = fs::rename(path, &bad).map(|_| format!("moved to {}", bad.display())).unwrap_or_else(|e| format!("could not be moved aside ({e})"));
            (Baseline::default(), Some(format!("{} was not valid ({e}), so it {moved} and nothing is marked as expected. Mark items again to rebuild it.", path.display())))
        }
    }
}

fn remember(notice: Option<String>) {
    if let Some(n) = notice {
        eprintln!("monitor-lizard: {n}");
        *NOTICE.lock().unwrap() = Some(n);
    }
}

pub fn load() -> Baseline {
    let (b, notice) = load_from(&file());
    remember(notice);
    b
}

/// The last problem found while reading the baseline, if any.
pub fn notice() -> Option<String> { NOTICE.lock().unwrap().clone() }

/// Writes through a temporary file and a rename, so a crash cannot leave half a list.
fn save_to(path: &Path, b: &Baseline) -> std::io::Result<()> {
    if let Some(d) = path.parent() { fs::create_dir_all(d)?; }
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    fs::write(&tmp, serde_json::to_string_pretty(&b.entries()).unwrap())?;
    fs::rename(&tmp, path)
}

/// Marks (`mark`) or unmarks one item. Returns whether the baseline changed. The change is written
/// to the history first and nothing is written if that fails, because the history is how a
/// wrongly marked item is found later.
pub fn change(kind: Kind, key: &str, mark: bool) -> Result<bool, String> {
    change_at(&file(), kind, key, mark, |k, key, mark| {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        crate::history::History::open().and_then(|h| h.record_baseline_change(now, k.name(), key, mark)).map_err(|e| e.to_string())
    })
}

fn change_at(path: &Path, kind: Kind, key: &str, mark: bool, audit: impl Fn(Kind, &str, bool) -> Result<(), String>) -> Result<bool, String> {
    validate(kind, key)?;
    let (mut b, notice) = load_from(path);
    remember(notice);
    let entry = typed(kind, key);
    if b.0.contains(&entry) == mark { return Ok(false); }
    audit(kind, key, mark).map_err(|e| format!("could not record the change in the history, so nothing was changed: {e}"))?;
    if mark { b.0.insert(entry); } else { b.0.remove(&entry); }
    save_to(path, &b).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ml-test-baseline-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("baseline.json")
    }
    fn ok_audit(_: Kind, _: &str, _: bool) -> Result<(), String> { Ok(()) }
    fn cleanup(p: &Path) { let _ = fs::remove_dir_all(p.parent().unwrap()); }

    #[test]
    fn an_old_untyped_file_loads_unchanged_with_every_entry_read_as_an_app() {
        let p = scratch("old");
        let text = r#"["/Applications/A.app","dev:claude-code","apple:/usr/bin/curl"]"#;
        fs::write(&p, text).unwrap();
        let (b, notice) = load_from(&p);
        assert!(notice.is_none());
        assert_eq!(b.apps(), ["/Applications/A.app", "dev:claude-code", "apple:/usr/bin/curl"].map(String::from).into_iter().collect());
        assert!(b.has(Kind::App, "dev:claude-code"), "a colon inside an app key is not a type prefix");
        assert_eq!(fs::read_to_string(&p).unwrap(), text, "loading does not rewrite the file");
        cleanup(&p);
    }

    #[test]
    fn the_first_write_converts_an_old_file_to_typed_entries_and_keeps_the_old_ones() {
        let p = scratch("convert");
        fs::write(&p, r#"["/Applications/A.app"]"#).unwrap();
        assert_eq!(change_at(&p, Kind::Startup, "/Library/LaunchAgents/x.plist", true, ok_audit), Ok(true));
        let on_disk: Vec<String> = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(on_disk, vec!["app:/Applications/A.app", "startup:/Library/LaunchAgents/x.plist"]);
        cleanup(&p);
    }

    #[test]
    fn each_kind_is_kept_apart() {
        let p = scratch("kinds");
        for (k, key) in [(Kind::App, "/Applications/A.app"), (Kind::Host, "/Applications/A.app|1.2.3.4"), (Kind::Startup, "/p.plist"), (Kind::Finding, "/Applications/A.app:exposed-listener")] {
            change_at(&p, k, key, true, ok_audit).unwrap();
        }
        let (b, _) = load_from(&p);
        assert_eq!(b.apps().len(), 1, "only the app entry is an approved app");
        assert!(b.has(Kind::Host, "/Applications/A.app|1.2.3.4") && !b.has(Kind::App, "/Applications/A.app|1.2.3.4"));
        assert!(b.has(Kind::Startup, "/p.plist") && b.has(Kind::Finding, "/Applications/A.app:exposed-listener"));
        cleanup(&p);
    }

    #[test]
    fn marking_the_same_key_twice_keeps_one_entry_and_audits_once() {
        let p = scratch("twice");
        let audited = RefCell::new(0);
        let audit = |_: Kind, _: &str, _: bool| { *audited.borrow_mut() += 1; Ok(()) };
        assert_eq!(change_at(&p, Kind::App, "/A.app", true, &audit), Ok(true));
        assert_eq!(change_at(&p, Kind::App, "/A.app", true, &audit), Ok(false));
        assert_eq!(load_from(&p).0.entries(), vec!["app:/A.app"]);
        assert_eq!(*audited.borrow(), 1);
        cleanup(&p);
    }

    #[test]
    fn unmarking_removes_the_entry_and_unmarking_an_absent_one_is_a_no_op() {
        let p = scratch("unmark");
        change_at(&p, Kind::Host, "k|1.1.1.1", true, ok_audit).unwrap();
        assert_eq!(change_at(&p, Kind::Host, "k|1.1.1.1", false, ok_audit), Ok(true));
        assert!(load_from(&p).0.entries().is_empty());
        assert_eq!(change_at(&p, Kind::Host, "k|1.1.1.1", false, ok_audit), Ok(false));
        cleanup(&p);
    }

    #[test]
    fn an_item_that_is_not_on_this_machine_can_be_marked_and_stays_until_unmarked() {
        let p = scratch("absent");
        change_at(&p, Kind::Startup, "/Library/LaunchAgents/never-existed.plist", true, ok_audit).unwrap();
        assert!(load_from(&p).0.has(Kind::Startup, "/Library/LaunchAgents/never-existed.plist"));
        cleanup(&p);
    }

    #[test]
    fn a_corrupt_file_is_moved_aside_and_an_empty_baseline_is_used_with_a_notice() {
        let p = scratch("corrupt");
        fs::write(&p, "{ not json").unwrap();
        let (b, notice) = load_from(&p);
        assert!(b.entries().is_empty());
        let n = notice.expect("the user is told");
        assert!(n.contains("baseline.json.bad"), "the notice names the backup: {n}");
        assert_eq!(fs::read_to_string(PathBuf::from(format!("{}.bad", p.display()))).unwrap(), "{ not json");
        assert!(!p.exists());
        cleanup(&p);
    }

    #[test]
    fn a_file_of_the_wrong_shape_counts_as_corrupt() {
        let p = scratch("shape");
        fs::write(&p, r#"{"approved":["x"]}"#).unwrap();
        assert!(load_from(&p).1.is_some());
        cleanup(&p);
    }

    #[test]
    fn an_unknown_kind_is_rejected_and_the_known_ones_parse() {
        assert_eq!(Kind::parse("planet"), None);
        assert_eq!(Kind::parse("host"), Some(Kind::Host));
        assert_eq!(parse_entry("host:a|b"), (Kind::Host, "a|b"));
        assert_eq!(parse_entry("/Applications/A.app"), (Kind::App, "/Applications/A.app"));
    }

    #[test]
    fn bad_keys_are_rejected_before_anything_is_written() {
        let p = scratch("invalid");
        for (k, key) in [(Kind::App, ""), (Kind::Host, "no-separator"), (Kind::Host, "|1.2.3.4"), (Kind::Host, "app|"), (Kind::Startup, "/a\nb")] {
            assert!(change_at(&p, k, key, true, ok_audit).is_err(), "{key:?} should be rejected");
        }
        assert!(change_at(&p, Kind::App, &"x".repeat(MAX_KEY_LEN + 1), true, ok_audit).is_err());
        assert!(!p.exists());
        cleanup(&p);
    }

    #[test]
    fn nothing_is_changed_when_the_history_cannot_record_it() {
        let p = scratch("audit-fails");
        let err = change_at(&p, Kind::App, "/A.app", true, |_, _, _| Err("disk full".into())).unwrap_err();
        assert!(err.contains("nothing was changed") && err.contains("disk full"), "{err}");
        assert!(!p.exists());
        cleanup(&p);
    }

    #[test]
    fn the_audit_sees_the_kind_the_bare_key_and_the_direction() {
        let p = scratch("audit-args");
        let seen = RefCell::new(Vec::new());
        let audit = |k: Kind, key: &str, mark: bool| { seen.borrow_mut().push((k.name(), key.to_string(), mark)); Ok(()) };
        change_at(&p, Kind::Host, "a|1.2.3.4", true, &audit).unwrap();
        change_at(&p, Kind::Host, "a|1.2.3.4", false, &audit).unwrap();
        assert_eq!(*seen.borrow(), vec![("host", "a|1.2.3.4".to_string(), true), ("host", "a|1.2.3.4".to_string(), false)]);
        cleanup(&p);
    }
}
