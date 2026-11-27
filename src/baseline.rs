//! The baseline: what the user has said is expected on this Mac. Each entry is a typed string,
//! `app:<group key>`, `host:<group key>|<address>`, `startup:<plist path>` or
//! `finding:<group key>:<rule>#<evidence>`, kept as a sorted JSON list in `baseline.json`.
use std::collections::{BTreeSet, HashSet};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
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
/// Most entries the baseline holds. A real Mac has tens; the cap stops a loop of requests from
/// growing the file, and marking beyond it is refused with a clear message.
pub const MAX_ENTRIES: usize = 5000;

/// Findings whose evidence is stable enough to fingerprint, so a mark covers only what the user saw:
/// the exposed ports. A new port changes the fingerprint and the finding returns. New hosts are not
/// here because the set shrinks as hosts age out of the window; those are marked host by host.
pub const MARKABLE_RULES: [&str; 2] = ["exposed-listener", "exposed-listener-apple"];
/// Rules that are never dismissable, with the reason shown to whoever tries.
const NEVER_EXPECTED: [(&str, &str); 2] = [
    ("unsigned-network", "an unsigned program using the network is never expected: it stays flagged and Suspicious"),
    ("debug-port", "an open remote-debugging port is never expected: any local process can drive that browser"),
];
/// Every rule id the rules can produce, so a typo in a key is refused instead of stored.
const ALL_RULES: [&str; 6] = ["exposed-listener", "exposed-listener-apple", "unsigned-network", "unreviewed", "debug-port", "new-remote"];

/// What `load` last had to tell the user, kept so the dashboard can keep showing it. The flag says
/// the file could not be read (as opposed to quarantined), which clears once it reads again.
static NOTICE: Mutex<Option<(String, bool)>> = Mutex::new(None);
/// Serializes writes inside this process; the lock file does the same across processes.
static WRITE_LOCK: Mutex<()> = Mutex::new(());
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn file() -> PathBuf { crate::paths::data_dir().join("baseline.json") }

pub fn typed(kind: Kind, key: &str) -> String { format!("{}:{key}", kind.name()) }

/// The key for one remote address of one app group.
pub fn host_key(group_key: &str, addr: &str) -> String { format!("{group_key}|{addr}") }

/// The key for one finding and the evidence it fired on.
pub fn finding_key(group_key: &str, rule: &str, fingerprint: &str) -> String { format!("{group_key}:{rule}#{fingerprint}") }

/// Splits a finding key into (group key, rule, fingerprint). The rule and fingerprint never contain
/// `#`, and the rule never contains `:`, so splitting from the right is unambiguous.
fn parse_finding_key(key: &str) -> Option<(&str, &str, &str)> {
    let (rest, fingerprint) = key.rsplit_once('#')?;
    let (group, rule) = rest.rsplit_once(':')?;
    Some((group, rule, fingerprint))
}

/// Splits `kind:key`. An entry with no known prefix is from before baselines were typed, when every
/// entry was an app group key. Group keys such as `dev:claude-code` or `apple:/usr/bin/x` contain a
/// colon but never start with one of the four prefixes, so they are not mistaken for typed entries.
pub fn parse_entry(entry: &str) -> (Kind, &str) {
    Kind::ALL.into_iter().find_map(|k| entry.strip_prefix(k.name()).and_then(|r| r.strip_prefix(':')).map(|key| (k, key))).unwrap_or((Kind::App, entry))
}

/// Control and invisible formatting characters (bidi overrides, zero-width marks, the byte order
/// mark) let a key read differently from what it is, so they are refused as well as plain controls.
fn is_unsafe_char(c: char) -> bool {
    c.is_control() || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')
}

/// Rejects keys that cannot name a real item. Existence on this machine is not checked: marking
/// something that is not here yet is allowed.
pub fn validate(kind: Kind, key: &str) -> Result<(), String> {
    if key.is_empty() { return Err("key is empty".into()); }
    if key.len() > MAX_KEY_LEN { return Err("key is too long".into()); }
    if key.chars().any(is_unsafe_char) { return Err("key contains a control or invisible formatting character".into()); }
    match kind {
        Kind::Host if !key.split_once('|').is_some_and(|(app, addr)| !app.is_empty() && !addr.is_empty()) => Err("a host key is <app key>|<address>".into()),
        Kind::Finding => {
            let Some((_, rule, _)) = parse_finding_key(key).filter(|(g, r, f)| !g.is_empty() && !r.is_empty() && !f.is_empty()) else {
                return Err("a finding key is <app key>:<rule>#<evidence>".into());
            };
            if let Some((_, why)) = NEVER_EXPECTED.iter().find(|(r, _)| *r == rule) { return Err((*why).into()); }
            if !ALL_RULES.contains(&rule) { return Err(format!("unknown rule {rule:?}")); }
            if !MARKABLE_RULES.contains(&rule) { return Err(format!("{rule} cannot be marked as a finding; mark its hosts one by one, or approve the app")); }
            Ok(())
        }
        _ => Ok(()),
    }
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

/// Why the file could not be used. `Corrupt` has text but is not valid; `Unreadable` could not be read at all.
enum Problem { Corrupt(String), Unreadable(String) }

/// Reads the file: Ok(None) when it does not exist, Err when it cannot be used.
fn parse_file(path: &Path) -> Result<Option<Baseline>, Problem> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Problem::Unreadable(format!("Could not read {}: {e}", path.display()))),
    };
    serde_json::from_str::<Vec<String>>(&text).map(|v| Some(Baseline::from_entries(v))).map_err(|e| Problem::Corrupt(format!("{} is not valid ({e})", path.display())))
}

/// Moves a corrupt file aside without overwriting an older backup: `.bad`, then `.bad-1`, `.bad-2`, ...
fn quarantine(path: &Path) -> String {
    let target = (0u32..).map(|n| PathBuf::from(if n == 0 { format!("{}.bad", path.display()) } else { format!("{}.bad-{n}", path.display()) })).find(|p| !p.exists()).unwrap();
    match fs::rename(path, &target) {
        Ok(()) => format!("it was moved to {}", target.display()),
        Err(e) => format!("it could not be moved aside ({e})"),
    }
}

fn corrupt_notice(what: &str, moved: &str) -> String {
    format!("{what}, so {moved}. Everything you marked as expected before is not in effect until you mark it again.")
}

fn remember(notice: String, unreadable: bool) {
    eprintln!("monitor-lizard: {notice}");
    *NOTICE.lock().unwrap() = Some((notice, unreadable));
}

/// The baseline for scanning. A corrupt file is quarantined here, which is why only startup and the
/// scans call it; `peek` is the read-only version for GET requests. An unreadable file gives an
/// empty baseline and a notice, and `change` refuses to write over it.
pub fn load() -> Baseline {
    let path = file();
    match parse_file(&path) {
        Ok(Some(b)) => {
            let mut n = NOTICE.lock().unwrap();
            if n.as_ref().is_some_and(|(_, unreadable)| *unreadable) { *n = None; }
            b
        }
        Ok(None) => Baseline::default(),
        Err(Problem::Corrupt(what)) => { let moved = quarantine(&path); remember(corrupt_notice(&what, &moved), false); Baseline::default() }
        Err(Problem::Unreadable(what)) => { remember(format!("{what}. Nothing is marked as expected until it can be read, and marking is paused."), true); Baseline::default() }
    }
}

/// Like `load` but touches nothing on disk, for GET requests.
pub fn peek() -> (Baseline, Option<String>) {
    match parse_file(&file()) {
        Ok(Some(b)) => (b, notice()),
        Ok(None) => (Baseline::default(), notice()),
        Err(Problem::Corrupt(what)) => (Baseline::default(), Some(corrupt_notice(&what, "it will be moved aside"))),
        Err(Problem::Unreadable(what)) => (Baseline::default(), Some(what)),
    }
}

/// The last problem found while reading the baseline, if any.
pub fn notice() -> Option<String> { NOTICE.lock().unwrap().as_ref().map(|(n, _)| n.clone()) }

/// Writes through a uniquely named temporary file that is flushed to disk, then renamed over the
/// real file, then the folder is flushed, so a crash leaves either the old list or the new one.
fn save_to(path: &Path, b: &Baseline) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    crate::paths::create_private_dir(dir)?;
    let tmp = PathBuf::from(format!("{}.{}-{}.tmp", path.display(), std::process::id(), TMP_COUNTER.fetch_add(1, Ordering::Relaxed)));
    let written = (|| {
        let mut f = crate::paths::create_new_private_file(&tmp)?;
        f.write_all(serde_json::to_string_pretty(&b.entries()).unwrap().as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        fs::File::open(dir)?.sync_all()
    })();
    if written.is_err() { let _ = fs::remove_file(&tmp); }
    written
}

/// Why a change was not made: the request was wrong (400) or the machine could not do it (500).
#[derive(Debug, PartialEq)]
pub enum ChangeError { Rejected(String), Failed(String) }

impl std::fmt::Display for ChangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { let (ChangeError::Rejected(m) | ChangeError::Failed(m)) = self; f.write_str(m) }
}

/// Marks (`mark`) or unmarks one item. Returns whether the baseline changed. The change is written
/// to the history first and nothing is written if that fails, because the history is how a
/// wrongly marked item is found later.
pub fn change(kind: Kind, key: &str, mark: bool) -> Result<bool, ChangeError> {
    change_at(&file(), kind, key, mark, |k, key, mark, note| {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        crate::history::History::open().and_then(|h| h.record_baseline_change(now, k.name(), key, mark, note)).map_err(|e| e.to_string())
    })
}

fn change_at(path: &Path, kind: Kind, key: &str, mark: bool, audit: impl Fn(Kind, &str, bool, &str) -> Result<(), String>) -> Result<bool, ChangeError> {
    validate(kind, key).map_err(ChangeError::Rejected)?;
    let _in_process = WRITE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let _across_processes = lock_file(path).map_err(|e| ChangeError::Failed(format!("could not lock the baseline for writing: {e}")))?;
    let mut b = match parse_file(path) {
        Ok(b) => b.unwrap_or_default(),
        Err(Problem::Corrupt(what)) => { let moved = quarantine(path); remember(corrupt_notice(&what, &moved), false); Baseline::default() }
        // Writing now would replace a file that may still hold the user's approvals.
        Err(Problem::Unreadable(what)) => return Err(ChangeError::Failed(format!("{what}; nothing was changed"))),
    };
    let entry = typed(kind, key);
    if b.0.contains(&entry) == mark { return Ok(false); }
    if mark && b.0.len() >= MAX_ENTRIES { return Err(ChangeError::Rejected(format!("the baseline already holds {MAX_ENTRIES} entries; unmark some first"))); }
    audit(kind, key, mark, "").map_err(|e| ChangeError::Failed(format!("could not record the change in the history, so nothing was changed: {e}")))?;
    if mark { b.0.insert(entry); } else { b.0.remove(&entry); }
    if let Err(e) = save_to(path, &b) {
        // The history already says the change happened; say that it did not, with the state it is really in.
        let _ = audit(kind, key, !mark, &format!("the change was not saved: {e}"));
        return Err(ChangeError::Failed(format!("could not write {}: {e}", path.display())));
    }
    Ok(true)
}

/// The lock file that sits next to the baseline.
fn lock_path(path: &Path) -> PathBuf { PathBuf::from(format!("{}.lock", path.display())) }

/// An exclusive lock on a file next to the baseline, released when the returned file is dropped.
fn lock_file(path: &Path) -> std::io::Result<fs::File> {
    if let Some(d) = path.parent() { crate::paths::create_private_dir(d)?; }
    let f = crate::paths::create_private_file(&lock_path(path))?;
    f.lock()?;
    Ok(f)
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
    fn ok_audit(_: Kind, _: &str, _: bool, _: &str) -> Result<(), String> { Ok(()) }
    fn cleanup(p: &Path) { let _ = fs::remove_dir_all(p.parent().unwrap()); }
    fn read(p: &Path) -> Baseline { parse_file(p).ok().flatten().unwrap_or_default() }
    const FINDING: &str = "/Applications/A.app:exposed-listener#3000";

    #[test]
    fn an_old_untyped_file_loads_unchanged_with_every_entry_read_as_an_app() {
        let p = scratch("old");
        let text = r#"["/Applications/A.app","dev:claude-code","apple:/usr/bin/curl"]"#;
        fs::write(&p, text).unwrap();
        let b = read(&p);
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
        for (k, key) in [(Kind::App, "/Applications/A.app"), (Kind::Host, "/Applications/A.app|1.2.3.4"), (Kind::Startup, "/p.plist"), (Kind::Finding, FINDING)] {
            change_at(&p, k, key, true, ok_audit).unwrap();
        }
        let b = read(&p);
        assert_eq!(b.apps().len(), 1, "only the app entry is an approved app");
        assert!(b.has(Kind::Host, "/Applications/A.app|1.2.3.4") && !b.has(Kind::App, "/Applications/A.app|1.2.3.4"));
        assert!(b.has(Kind::Startup, "/p.plist") && b.has(Kind::Finding, FINDING));
        cleanup(&p);
    }

    #[test]
    fn marking_the_same_key_twice_keeps_one_entry_and_audits_once() {
        let p = scratch("twice");
        let audited = RefCell::new(0);
        let audit = |_: Kind, _: &str, _: bool, _: &str| { *audited.borrow_mut() += 1; Ok(()) };
        assert_eq!(change_at(&p, Kind::App, "/A.app", true, &audit), Ok(true));
        assert_eq!(change_at(&p, Kind::App, "/A.app", true, &audit), Ok(false));
        assert_eq!(read(&p).entries(), vec!["app:/A.app"]);
        assert_eq!(*audited.borrow(), 1);
        cleanup(&p);
    }

    #[test]
    fn unmarking_removes_the_entry_and_unmarking_an_absent_one_is_a_no_op() {
        let p = scratch("unmark");
        change_at(&p, Kind::Host, "k|1.1.1.1", true, ok_audit).unwrap();
        assert_eq!(change_at(&p, Kind::Host, "k|1.1.1.1", false, ok_audit), Ok(true));
        assert!(read(&p).entries().is_empty());
        assert_eq!(change_at(&p, Kind::Host, "k|1.1.1.1", false, ok_audit), Ok(false));
        cleanup(&p);
    }

    #[test]
    fn an_item_that_is_not_on_this_machine_can_be_marked_and_stays_until_unmarked() {
        let p = scratch("absent");
        change_at(&p, Kind::Startup, "/Library/LaunchAgents/never-existed.plist", true, ok_audit).unwrap();
        assert!(read(&p).has(Kind::Startup, "/Library/LaunchAgents/never-existed.plist"));
        cleanup(&p);
    }

    #[test]
    fn a_corrupt_file_is_moved_aside_never_over_an_older_backup_and_the_user_is_told() {
        let p = scratch("corrupt");
        let bad = PathBuf::from(format!("{}.bad", p.display()));
        fs::write(&bad, "older backup").unwrap();
        fs::write(&p, "{ not json").unwrap();
        // A read-only look touches nothing.
        let Err(Problem::Corrupt(_)) = parse_file(&p) else { panic!("should be corrupt") };
        assert!(p.exists());
        let moved = quarantine(&p);
        assert!(moved.contains("baseline.json.bad-1"), "{moved}");
        assert_eq!(fs::read_to_string(&bad).unwrap(), "older backup", "the older backup is untouched");
        assert_eq!(fs::read_to_string(format!("{}-1", bad.display())).unwrap(), "{ not json");
        assert!(corrupt_notice("x", &moved).contains("not in effect"));
        cleanup(&p);
    }

    #[test]
    fn a_write_over_a_corrupt_file_quarantines_it_and_starts_from_empty() {
        let p = scratch("corrupt-write");
        fs::write(&p, r#"{"approved":["x"]}"#).unwrap();
        assert_eq!(change_at(&p, Kind::App, "/A.app", true, ok_audit), Ok(true));
        assert_eq!(read(&p).entries(), vec!["app:/A.app"]);
        assert!(PathBuf::from(format!("{}.bad", p.display())).exists());
        cleanup(&p);
    }

    #[test]
    fn an_unreadable_file_refuses_writes_and_is_never_replaced() {
        use std::os::unix::fs::PermissionsExt;
        let p = scratch("unreadable");
        fs::write(&p, r#"["/A.app"]"#).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read_to_string(&p).is_ok() { cleanup(&p); return; } // running as root: cannot make a file unreadable
        let err = change_at(&p, Kind::App, "/B.app", true, ok_audit).unwrap_err();
        assert!(matches!(err, ChangeError::Failed(ref m) if m.contains("nothing was changed")), "{err}");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), r#"["/A.app"]"#, "the original survived");
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
        for (k, key) in [(Kind::App, ""), (Kind::Host, "no-separator"), (Kind::Host, "|1.2.3.4"), (Kind::Host, "app|"), (Kind::Startup, "/a\nb"),
            (Kind::Startup, "/a\u{202E}b"), (Kind::Startup, "/a\u{200B}b"), (Kind::App, "\u{FEFF}/A.app"), (Kind::Startup, "/a\u{2066}b")] {
            assert!(matches!(change_at(&p, k, key, true, ok_audit), Err(ChangeError::Rejected(_))), "{key:?} should be rejected");
        }
        assert!(change_at(&p, Kind::App, &"x".repeat(MAX_KEY_LEN + 1), true, ok_audit).is_err());
        assert!(!p.exists());
        cleanup(&p);
    }

    #[test]
    fn finding_keys_are_checked_against_the_real_rules_at_write_time() {
        let e = |key: &str| validate(Kind::Finding, key).unwrap_err();
        assert!(e("/A.app:unsigned-network#x").contains("never expected"));
        assert!(e("/A.app:debug-port#9222").contains("never expected"));
        assert!(e("/A.app:made-up-rule#x").contains("unknown rule"));
        assert!(e("/A.app:unreviewed#x").contains("cannot be marked"));
        assert!(e("/A.app:new-remote#1.1.1.1").contains("mark its hosts"));
        assert!(e("/A.app:exposed-listener").contains("<app key>:<rule>#<evidence>"), "evidence is required");
        assert!(e("/A.app:exposed-listener#").contains("<evidence>"));
        assert!(validate(Kind::Finding, FINDING).is_ok());
        assert!(validate(Kind::Finding, "/A.app:exposed-listener-apple#7000").is_ok());
        let p = scratch("never");
        assert!(matches!(change_at(&p, Kind::Finding, "/A.app:unsigned-network#x", true, ok_audit), Err(ChangeError::Rejected(_))));
        assert!(!p.exists(), "nothing was written or audited");
        cleanup(&p);
    }

    #[test]
    fn marking_beyond_the_entry_cap_is_rejected_but_unmarking_still_works() {
        let p = scratch("cap");
        let entries: Vec<String> = (0..MAX_ENTRIES).map(|n| format!("app:/A{n}.app")).collect();
        fs::write(&p, serde_json::to_string(&entries).unwrap()).unwrap();
        assert!(matches!(change_at(&p, Kind::App, "/extra.app", true, ok_audit), Err(ChangeError::Rejected(m)) if m.contains("5000")));
        assert_eq!(change_at(&p, Kind::App, "/A0.app", false, ok_audit), Ok(true));
        cleanup(&p);
    }

    #[test]
    fn nothing_is_changed_when_the_history_cannot_record_it() {
        let p = scratch("audit-fails");
        let err = change_at(&p, Kind::App, "/A.app", true, |_, _, _, _| Err("disk full".into())).unwrap_err();
        assert!(matches!(&err, ChangeError::Failed(m) if m.contains("nothing was changed") && m.contains("disk full")), "{err}");
        assert!(!p.exists());
        cleanup(&p);
    }

    #[test]
    fn the_audit_sees_the_kind_the_bare_key_and_the_direction() {
        let p = scratch("audit-args");
        let seen = RefCell::new(Vec::new());
        let audit = |k: Kind, key: &str, mark: bool, note: &str| { seen.borrow_mut().push((k.name(), key.to_string(), mark, note.to_string())); Ok(()) };
        change_at(&p, Kind::Host, "a|1.2.3.4", true, &audit).unwrap();
        change_at(&p, Kind::Host, "a|1.2.3.4", false, &audit).unwrap();
        assert_eq!(*seen.borrow(), vec![("host", "a|1.2.3.4".to_string(), true, String::new()), ("host", "a|1.2.3.4".to_string(), false, String::new())]);
        cleanup(&p);
    }

    #[test]
    fn a_failed_write_after_the_audit_adds_a_second_row_saying_the_change_did_not_happen() {
        use std::os::unix::fs::PermissionsExt;
        let p = scratch("save-fails");
        change_at(&p, Kind::App, "/First.app", true, ok_audit).unwrap(); // creates the file and its lock
        let dir = p.parent().unwrap().to_path_buf();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
        let seen = RefCell::new(Vec::new());
        let audit = |_: Kind, key: &str, mark: bool, note: &str| { seen.borrow_mut().push((key.to_string(), mark, note.to_string())); Ok(()) };
        let err = change_at(&p, Kind::App, "/A.app", true, &audit);
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        if fs::File::create(dir.join("probe")).is_ok() && err.is_ok() { cleanup(&p); return; } // running as root: the directory was still writable
        assert!(matches!(err, Err(ChangeError::Failed(ref m)) if m.contains("could not write")), "{err:?}");
        let rows = seen.borrow();
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert_eq!(rows[0], ("/A.app".to_string(), true, String::new()));
        assert!(!rows[1].1 && rows[1].2.contains("not saved"), "{:?}", rows[1]);
        assert_eq!(read(&p).entries(), vec!["app:/First.app"], "the file is unchanged");
        cleanup(&p);
    }

    #[test]
    fn the_file_is_owner_only_and_leaves_no_temporary_file() {
        use std::os::unix::fs::PermissionsExt;
        let p = scratch("mode");
        change_at(&p, Kind::App, "/A.app", true, ok_audit).unwrap();
        assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        let tmp = fs::read_dir(p.parent().unwrap()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).count();
        assert_eq!(tmp, 0);
        cleanup(&p);
    }

    #[test]
    fn concurrent_marks_from_threads_all_land() {
        let p = scratch("threads");
        let handles: Vec<_> = (0..8).map(|n| { let p = p.clone(); std::thread::spawn(move || change_at(&p, Kind::App, &format!("/A{n}.app"), true, ok_audit).unwrap()) }).collect();
        for h in handles { h.join().unwrap(); }
        assert_eq!(read(&p).entries().len(), 8);
        cleanup(&p);
    }

    // Another process writing the baseline is kept out only by the lock file. A flock belongs to
    // the open file, so a second handle in this process stands in for that other process. The
    // tests below start no process and every wait has a limit, so a broken lock fails an
    // assertion instead of hanging the test run.

    /// A second handle on the lock file, as another process would open it.
    fn other_handle(p: &Path) -> fs::File { fs::OpenOptions::new().write(true).open(lock_path(p)).unwrap() }

    #[test]
    fn a_held_baseline_lock_refuses_a_second_holder_until_it_is_dropped() {
        let p = scratch("lock-excludes");
        let first = lock_file(&p).unwrap();
        let second = other_handle(&p);
        assert!(matches!(second.try_lock(), Err(fs::TryLockError::WouldBlock)), "a second handle got the baseline lock while the first still held it");
        drop(first);
        assert!(crate::paths::try_lock_within(&second, std::time::Duration::from_secs(5)), "the baseline lock was not released within 5 s of its holder being dropped");
        cleanup(&p);
    }

    #[test]
    fn a_change_waits_for_a_baseline_lock_held_elsewhere_and_reads_the_file_only_after_it_gets_the_lock() {
        use std::sync::mpsc;
        use std::time::Duration;
        let p = scratch("lock-waits");
        let held = lock_file(&p).unwrap();
        let (done, finished) = mpsc::channel();
        let writer = { let p = p.clone(); std::thread::spawn(move || { let _ = done.send(change_at(&p, Kind::App, "/Waited.app", true, ok_audit)); }) };
        // If the change ignored the lock it would finish well within this time.
        match finished.recv_timeout(Duration::from_millis(300)) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Ok(r) => panic!("the change finished ({r:?}) while another holder had the baseline lock"),
            Err(mpsc::RecvTimeoutError::Disconnected) => panic!("the change thread panicked"),
        }
        // Another process writes a mark while it holds the lock. A change that read the file before
        // taking the lock would write back the old list and lose this mark.
        save_to(&p, &Baseline::from_entries(vec![typed(Kind::App, "/Other.app")])).unwrap();
        drop(held);
        let result = finished.recv_timeout(Duration::from_secs(10)).expect("the change did not finish within 10 s of the lock being released, or its thread panicked");
        writer.join().unwrap();
        assert_eq!(result, Ok(true));
        let after = read(&p);
        assert!(after.has(Kind::App, "/Waited.app"), "the mark made after the wait is on disk");
        assert!(after.has(Kind::App, "/Other.app"), "the change read the file before it had the lock, so it lost the mark written while it waited");
        cleanup(&p);
    }

    #[test]
    fn the_baseline_lock_is_still_held_between_the_read_and_the_write() {
        let p = scratch("lock-held");
        // The lock file must exist for the audit to open a second handle on it. It is made without
        // locking it, so no earlier holder can be what refuses the audit.
        crate::paths::create_private_file(&lock_path(&p)).unwrap();
        let refused = RefCell::new(None);
        let audit = |_: Kind, _: &str, _: bool, _: &str| { *refused.borrow_mut() = Some(matches!(other_handle(&p).try_lock(), Err(fs::TryLockError::WouldBlock))); Ok(()) };
        assert_eq!(change_at(&p, Kind::App, "/A.app", true, audit), Ok(true));
        assert_eq!(*refused.borrow(), Some(true), "another process could take the baseline lock between the read and the write");
        cleanup(&p);
    }

    #[test]
    fn a_new_folder_made_by_a_save_or_a_lock_is_0700_and_the_files_are_0600() {
        use std::os::unix::fs::PermissionsExt;
        let m = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        let root = std::env::temp_dir().join(format!("monlizard-baseline-modes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (a, b) = (root.join("a").join("baseline.json"), root.join("b").join("baseline.json"));
        save_to(&a, &Baseline::default()).unwrap();
        assert_eq!((m(a.parent().unwrap()), m(&a)), (0o700, 0o600));
        let _lock = lock_file(&b).unwrap();
        assert_eq!((m(b.parent().unwrap()), m(&lock_path(&b))), (0o700, 0o600));
        let _ = fs::remove_dir_all(&root);
    }
}
