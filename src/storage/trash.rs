//! Moving scanned items to the Trash: the only way this app removes files. Every guard here runs
//! on the server, because the page that asks is untrusted. A request carries ids from the latest
//! scan, never paths, and each item is checked again against the disk as it is now.
//!
//! The order for a real move is fixed: check the item, write an audit row saying it is about to move
//! (`Audit::attempting`), check the path once more, move, then record the result. If the audit row
//! cannot be written the item is not moved.
use super::{walk::{self, Limits}, Item, Label, MAX_ENTRIES_PER_LOCATION};
use crate::sampler::Reading;
use serde::Serialize;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::{Duration, Instant};

/// The most ids one request may name.
pub const MAX_IDS: usize = 50;
/// Re-measuring one item must not hold a request for long.
const REMEASURE_BUDGET: Duration = Duration::from_secs(10);
/// The whole request, checks and moves together. Items not reached by then are reported as not attempted.
pub const REQUEST_BUDGET: Duration = Duration::from_secs(45);

/// The text shown to the user for each refusal. `Code` is the machine-readable twin.
pub mod reason {
    pub const GONE: &str = "no longer exists; rescan";
    pub const SYMLINK: &str = "is a symbolic link now, so it was not moved";
    pub const OUTSIDE: &str = "is outside the home folder and /Applications, so it was not moved";
    pub const YOURS: &str = "is labelled yours and needs the second confirmation";
    pub const UNMEASURED: &str = "was not measured completely, so a change cannot be ruled out; rescan";
    pub const CHANGED: &str = "changed since the scan; rescan";
    pub const NOT_OWNED: &str = "is owned by another user (for example root), so this app cannot move it to your Trash";
    pub const MOUNT_INSIDE: &str = "has another volume mounted inside it, so it was not moved";
    pub const MOUNT_POINT: &str = "is a mount point (another volume), so it was not moved";
    pub const NOT_PREVIEWED: &str = "was not part of the preview this confirmation was issued for, so it was not moved; preview again";
    pub const NOT_ATTEMPTED: &str = "not attempted: an earlier move in this request failed";
    pub const TIME_BUDGET: &str = "not attempted: the time allowed for this request ran out; ask again";
    pub const AUDIT: &str = "not attempted: the audit log could not be written";
}

/// Why an item was not moved, as a stable word for the page and the audit log.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Code { Gone, Symlink, Outside, NeedsYoursConfirmation, Unmeasured, Changed, NotOwned, MountInside, NotPreviewed, Unreadable, TimeBudget, EarlierMoveFailed, AuditFailed, MoveNotPermitted, MoveFailed }

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Gone => "gone", Code::Symlink => "symlink", Code::Outside => "outside_allowed_folders",
            Code::NeedsYoursConfirmation => "needs_yours_confirmation", Code::Unmeasured => "unmeasured",
            Code::Changed => "changed", Code::NotOwned => "not_owned", Code::MountInside => "mount_inside", Code::NotPreviewed => "not_previewed",
            Code::Unreadable => "unreadable", Code::TimeBudget => "time_budget", Code::EarlierMoveFailed => "earlier_move_failed",
            Code::AuditFailed => "audit_failed", Code::MoveNotPermitted => "move_not_permitted", Code::MoveFailed => "move_failed",
        }
    }
}

/// What the mover reports when it cannot move an item.
#[derive(Debug, Clone, PartialEq)]
pub enum MoveError {
    NotFound,
    NotPermitted(String),
    Other(String),
}

impl std::fmt::Display for MoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoveError::NotFound => f.write_str("no longer exists"),
            MoveError::NotPermitted(s) | MoveError::Other(s) => f.write_str(s),
        }
    }
}

impl MoveError {
    fn code(&self) -> Code {
        match self { MoveError::NotFound => Code::Gone, MoveError::NotPermitted(_) => Code::MoveNotPermitted, MoveError::Other(_) => Code::MoveFailed }
    }
}

pub type Mover<'a> = &'a dyn Fn(&Path) -> Result<(), MoveError>;

/// Nanoseconds since the epoch of the path's own modification (a symlink is not followed).
pub fn modified_ns(p: &Path) -> Option<u64> {
    let m = std::fs::symlink_metadata(p).ok()?;
    Some((m.mtime().max(0) as u64).saturating_mul(1_000_000_000).saturating_add(m.mtime_nsec().max(0) as u64))
}

/// Ids are the 16 hex digits `item_id` makes. Anything else is refused before a lookup.
pub fn parse_ids(raw: &str) -> Result<Vec<String>, String> {
    let mut ids: Vec<String> = Vec::new();
    for id in raw.split(',').filter(|s| !s.is_empty()) {
        if id.len() != 16 || !id.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(format!("bad id '{}'", id.chars().take(20).collect::<String>())); }
        if !ids.iter().any(|i| i == id) { ids.push(id.to_string()); }
    }
    if ids.is_empty() || ids.len() > MAX_IDS { return Err(format!("expected 1 to {MAX_IDS} ids")); }
    Ok(ids)
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status { WouldMove, Moved, Refused, Failed, NotAttempted }

impl Status {
    pub fn as_str(self) -> &'static str {
        match self { Status::WouldMove => "would_move", Status::Moved => "moved", Status::Refused => "refused", Status::Failed => "failed", Status::NotAttempted => "not_attempted" }
    }
}

/// What happened to one requested item.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ItemResult {
    pub id: String,
    pub path: String,
    pub label: Label,
    pub status: Status,
    pub code: Option<Code>,
    pub reason: Option<String>,
    pub bytes: Option<u64>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Entry { pub id: String, pub path: String, pub bytes: u64, pub label: Label }

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Refusal { pub id: String, pub path: String, pub reason: String, pub code: Code, pub status: Status }

#[derive(Serialize, Debug, Default, PartialEq)]
pub struct Counts { pub would_move: usize, pub moved: usize, pub refused: usize, pub failed: usize, pub not_attempted: usize }

/// What a request did, one result per requested id in request order. In a dry run nothing was touched.
#[derive(Debug, Default)]
pub struct Outcome {
    pub dry_run: bool,
    pub items: Vec<ItemResult>,
    /// Bytes moved, or that would move in a dry run.
    pub bytes: u64,
    /// Set when the audit log could not be written during the run.
    pub audit_error: Option<String>,
}

impl Outcome {
    pub fn counts(&self) -> Counts {
        let n = |s| self.items.iter().filter(|i| i.status == s).count();
        Counts { would_move: n(Status::WouldMove), moved: n(Status::Moved), refused: n(Status::Refused), failed: n(Status::Failed), not_attempted: n(Status::NotAttempted) }
    }
    /// True when every requested item moved (or would, in a dry run) and the audit log was written.
    pub fn ok(&self) -> bool { self.audit_error.is_none() && self.items.iter().all(|i| matches!(i.status, Status::Moved | Status::WouldMove)) }
    pub fn moved(&self) -> Vec<Entry> {
        self.items.iter().filter(|i| matches!(i.status, Status::Moved | Status::WouldMove))
            .map(|i| Entry { id: i.id.clone(), path: i.path.clone(), bytes: i.bytes.unwrap_or(0), label: i.label }).collect()
    }
    pub fn refused(&self) -> Vec<Refusal> {
        self.items.iter().filter(|i| !matches!(i.status, Status::Moved | Status::WouldMove))
            .filter_map(|i| Some(Refusal { id: i.id.clone(), path: i.path.clone(), reason: i.reason.clone()?, code: i.code?, status: i.status })).collect()
    }
}

/// Where a real run records itself. `attempting` runs before an item is moved and must succeed for it
/// to move; `finished` records the result and may fail without undoing anything.
pub trait Audit {
    fn attempting(&mut self, item: &Item) -> Result<(), String>;
    fn finished(&mut self, result: &ItemResult) -> Result<(), String>;
}

/// For a dry run, which records nothing.
pub struct NoAudit;
impl Audit for NoAudit {
    fn attempting(&mut self, _: &Item) -> Result<(), String> { Ok(()) }
    fn finished(&mut self, _: &ItemResult) -> Result<(), String> { Ok(()) }
}

/// What an item is checked against.
pub struct Policy<'a> {
    pub home: &'a Path,
    pub apps: &'a Path,
    /// The user running this app: an item owned by anyone else is refused.
    pub uid: u32,
    pub confirm_yours: bool,
    /// For a real move: the ids the preview showed as movable. Any other item is refused, so what moves
    /// is never more than what was previewed. None only for a preview.
    pub previewed: Option<&'a [String]>,
    /// When the request's time runs out; items not reached are not attempted.
    pub deadline: Instant,
}

type Refuse = (Code, &'static str);

/// The checks of the path itself, cheap enough to repeat right before the move.
fn path_checks(item: &Item, pol: &Policy) -> Result<(), Refuse> {
    let path = Path::new(&item.path);
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err((Code::Gone, reason::GONE)),
        Err(e) => return Err((Code::Unreadable, walk::io_reason(&e))),
    };
    if meta.file_type().is_symlink() { return Err((Code::Symlink, reason::SYMLINK)); }
    // The path must be its own canonical form: no symlinked folder above it, no `..` in it, and the
    // same letter case the disk has (a case-insensitive volume would otherwise accept `library`).
    let canonical = path.canonicalize().map_err(|e| (Code::Unreadable, walk::io_reason(&e)))?;
    let inside = |root: &Path| root.canonicalize().is_ok_and(|r| canonical != r && canonical.starts_with(&r));
    if canonical != path || !(inside(pol.home) || inside(pol.apps)) { return Err((Code::Outside, reason::OUTSIDE)); }
    if meta.uid() != pol.uid { return Err((Code::NotOwned, reason::NOT_OWNED)); }
    // An item that is itself a mount point lives on another device than the folder holding it.
    if path.parent().and_then(|p| std::fs::symlink_metadata(p).ok()).is_some_and(|p| walk::crosses(p.dev(), meta.dev())) { return Err((Code::MountInside, reason::MOUNT_POINT)); }
    if item.label == Label::Yours && !pol.confirm_yours { return Err((Code::NeedsYoursConfirmation, reason::YOURS)); }
    if !matches!(item.size, Reading::Value(_)) || !item.complete { return Err((Code::Unmeasured, reason::UNMEASURED)); }
    if item.modified_ns.is_none() || modified_ns(path) != item.modified_ns { return Err((Code::Changed, reason::CHANGED)); }
    Ok(())
}

/// Why this item must not be moved right now, or its size when it may. The cheap checks of the path
/// itself come first and the re-measure last.
fn check(item: &Item, pol: &Policy) -> Result<u64, Refuse> {
    path_checks(item, pol)?;
    let Reading::Value(scanned) = &item.size else { return Err((Code::Unmeasured, reason::UNMEASURED)) };
    let now = Instant::now();
    if now >= pol.deadline { return Err((Code::TimeBudget, reason::TIME_BUDGET)); }
    let lim = Limits { deadline: (now + REMEASURE_BUDGET).min(pol.deadline), max_entries: MAX_ENTRIES_PER_LOCATION };
    match walk::walk(Path::new(&item.path), lim) {
        None => Err((Code::Gone, reason::GONE)),
        Some(Ok(w)) if w.stop == Some(walk::Stop::Time) && Instant::now() >= pol.deadline => Err((Code::TimeBudget, reason::TIME_BUDGET)),
        Some(Ok(w)) if w.crossed > 0 => Err((Code::MountInside, reason::MOUNT_INSIDE)),
        // Allocated bytes are in whole blocks and a folder's own time ignores edits inside a file, so
        // the exact length and the entry count must match as well. A same-length overwrite still passes;
        // the Trash is the backstop for that.
        Some(Ok(w)) if w.stop.is_none() && w.unreadable == 0 && w.allocated == *scanned && Some(w.apparent) == item.apparent_bytes && w.entries == item.entries => Ok(*scanned),
        Some(Ok(w)) if w.stop.is_some() || w.unreadable > 0 => Err((Code::Unmeasured, reason::UNMEASURED)),
        Some(Err(why)) => Err((Code::Unreadable, why)),
        Some(Ok(_)) => Err((Code::Changed, reason::CHANGED)),
    }
}

fn result(item: &Item, status: Status, code: Option<Code>, why: Option<String>, bytes: Option<u64>) -> ItemResult {
    ItemResult { id: item.id.clone(), path: item.path.clone(), label: item.label, status, code, reason: why, bytes }
}

/// The state of one request while it runs.
struct Runner<'a> {
    pol: &'a Policy<'a>,
    mover: Mover<'a>,
    audit: &'a mut dyn Audit,
    /// Why every remaining item is not attempted, once the request has stopped.
    stopped: Option<(Code, &'static str)>,
}

impl Runner<'_> {
    fn not_attempted(&mut self, item: &Item, (code, why): (Code, &'static str)) -> ItemResult {
        self.stopped = Some((code, why));
        result(item, Status::NotAttempted, Some(code), Some(why.to_string()), None)
    }

    fn item(&mut self, item: &Item, dry: bool) -> ItemResult {
        if let Some(s) = self.stopped { return self.not_attempted(item, s); }
        if Instant::now() >= self.pol.deadline { return self.not_attempted(item, (Code::TimeBudget, reason::TIME_BUDGET)); }
        match check(item, self.pol) {
            Err((Code::TimeBudget, why)) => self.not_attempted(item, (Code::TimeBudget, why)),
            Err((code, why)) => result(item, Status::Refused, Some(code), Some(why.to_string()), None),
            Ok(bytes) if dry => result(item, Status::WouldMove, None, None, Some(bytes)),
            Ok(_) if self.pol.previewed.is_some_and(|p| !p.contains(&item.id)) => result(item, Status::Refused, Some(Code::NotPreviewed), Some(reason::NOT_PREVIEWED.to_string()), None),
            Ok(bytes) => self.move_item(item, bytes),
        }
    }

    /// The audited part of a real move: row first, path check again, then the move.
    fn move_item(&mut self, item: &Item, bytes: u64) -> ItemResult {
        if self.audit.attempting(item).is_err() { return self.not_attempted(item, (Code::AuditFailed, reason::AUDIT)); }
        // The time between the first check and now is where a path could be swapped for a link.
        if let Err((code, why)) = path_checks(item, self.pol) { return result(item, Status::Refused, Some(code), Some(why.to_string()), None); }
        match (self.mover)(Path::new(&item.path)) {
            Ok(()) => result(item, Status::Moved, None, None, Some(bytes)),
            Err(e) => { self.stopped = Some((Code::EarlierMoveFailed, reason::NOT_ATTEMPTED)); result(item, Status::Failed, Some(e.code()), Some(e.to_string()), None) }
        }
    }
}

/// Checks every item, then (unless `dry`) moves the ones that passed with `mover`, in request order.
/// A refusal of one item never stops the others. A failed move, a failed audit write or the end of the
/// time budget stops the request: the rest are listed as not attempted, so a half-done batch never
/// looks like it finished, and nothing is moved without its audit row.
pub fn run(items: &[Item], pol: &Policy, dry: bool, mover: Mover, audit: &mut dyn Audit) -> Outcome {
    let mut out = Outcome { dry_run: dry, ..Outcome::default() };
    let mut runner = Runner { pol, mover, audit, stopped: None };
    for item in items {
        let r = runner.item(item, dry);
        if matches!(r.status, Status::Moved | Status::WouldMove) { out.bytes = out.bytes.saturating_add(r.bytes.unwrap_or(0)); }
        if !dry { if let Err(e) = runner.audit.finished(&r) { out.audit_error.get_or_insert(e); } }
        out.items.push(r);
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::storage::{item_id, Cache, Category, Rule, Storage};
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;

    /// Removes the temp folders a test made when its thread ends, pass or fail. A folder that
    /// something is still mounted on is skipped (see `keep`): walking into a mount would delete the
    /// mounted volume's files.
    #[derive(Default)]
    struct Cleanup { made: Vec<PathBuf>, keep: Vec<PathBuf> }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            for p in &self.made { if !self.keep.iter().any(|k| k.starts_with(p)) { let _ = fs::remove_dir_all(p); } }
        }
    }
    thread_local!(static CLEANUP: RefCell<Cleanup> = RefCell::new(Cleanup::default()));
    /// Excludes everything above `inside` from the end-of-test cleanup.
    pub(crate) fn keep(inside: &Path) { CLEANUP.with(|c| c.borrow_mut().keep.push(inside.to_path_buf())); }

    pub(crate) fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ml-trash-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        let p = p.canonicalize().unwrap();
        CLEANUP.with(|c| c.borrow_mut().made.push(p.clone()));
        p
    }

    /// An item as a scan of `path` would have produced it.
    pub(crate) fn scanned(path: &Path, label: Label) -> Item {
        let text = path.display().to_string();
        let w = walk::walk(path, Limits { deadline: Instant::now() + Duration::from_secs(5), max_entries: 1000 }).unwrap().unwrap();
        Item { id: item_id(&text), path: text, category: Category::Caches, label, rule: Rule::AppCache, size: Reading::Value(w.allocated), apparent_bytes: Some(w.apparent), entries: w.entries, complete: true, unreadable: 0, measured_at: 0, reused: false, stale: false, modified_ns: modified_ns(path) }
    }

    fn me() -> u32 { crate::storage::current_uid() }

    fn pol<'a>(home: &'a Path, apps: &'a Path) -> Policy<'a> {
        Policy { home, apps, uid: me(), confirm_yours: false, previewed: None, deadline: Instant::now() + Duration::from_secs(60) }
    }

    fn go(items: &[Item], p: &Policy, dry: bool, mover: Mover) -> Outcome { run(items, p, dry, mover, &mut NoAudit) }

    /// A mover that records what it was given and removes it, in place of the Trash.
    fn recording(log: &RefCell<Vec<PathBuf>>) -> impl Fn(&Path) -> Result<(), MoveError> + '_ {
        move |p| { log.borrow_mut().push(p.to_path_buf()); fs::remove_dir_all(p).or_else(|_| fs::remove_file(p)).map_err(|e| MoveError::Other(e.to_string())) }
    }

    fn never(_: &Path) -> Result<(), MoveError> { panic!("must not move") }

    pub(crate) fn cache_dir(home: &Path, name: &str) -> PathBuf {
        let d = home.join("Library/Caches").join(name);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("blob"), vec![1u8; 40_000]).unwrap();
        d
    }

    fn item_bytes(i: &Item) -> u64 { if let Reading::Value(b) = i.size { b } else { 0 } }

    fn why(o: &Outcome, i: usize) -> (Status, Option<Code>, &str) { (o.items[i].status, o.items[i].code, o.items[i].reason.as_deref().unwrap_or("")) }

    #[test]
    fn a_safe_item_moves_and_is_reported_with_its_size() {
        let (home, apps) = (temp("moves"), temp("moves-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        let log = RefCell::new(vec![]);
        let o = go(&[item.clone()], &pol(&home, &apps), false, &recording(&log));
        assert_eq!((o.counts().moved, o.refused().len(), o.bytes), (1, 0, item_bytes(&item)));
        assert!(o.ok());
        assert_eq!(*log.borrow(), vec![d.clone()]);
        assert!(!d.exists());
    }

    #[test]
    fn a_dry_run_reports_and_touches_nothing() {
        let (home, apps) = (temp("dry"), temp("dry-apps"));
        let d = cache_dir(&home, "app");
        let log = RefCell::new(vec![]);
        let o = go(&[scanned(&d, Label::Safe)], &pol(&home, &apps), true, &recording(&log));
        assert!(o.dry_run && o.counts().would_move == 1 && o.bytes > 0 && o.ok());
        assert!(log.borrow().is_empty() && d.exists());
    }

    #[test]
    fn a_path_replaced_by_a_symlink_after_the_scan_is_refused() {
        let (home, apps) = (temp("symlink"), temp("symlink-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        let precious = home.join("precious");
        fs::create_dir_all(&precious).unwrap();
        fs::remove_dir_all(&d).unwrap();
        std::os::unix::fs::symlink(&precious, &d).unwrap();
        let o = go(&[item], &pol(&home, &apps), false, &never);
        assert_eq!(why(&o, 0), (Status::Refused, Some(Code::Symlink), reason::SYMLINK), "{o:?}");
        assert!(precious.exists());
    }

    /// Swaps the item for a link to something precious after the first check passed and before the
    /// move: the second check, right before the mover, must catch it.
    struct SwapOnAttempt { swap: Box<dyn Fn()> }
    impl Audit for SwapOnAttempt {
        fn attempting(&mut self, _: &Item) -> Result<(), String> { (self.swap)(); Ok(()) }
        fn finished(&mut self, _: &ItemResult) -> Result<(), String> { Ok(()) }
    }

    #[test]
    fn a_path_swapped_for_a_symlink_between_the_check_and_the_move_is_refused() {
        let (home, apps) = (temp("toctou"), temp("toctou-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        let precious = home.join("precious");
        fs::create_dir_all(&precious).unwrap();
        fs::write(precious.join("keep"), "x").unwrap();
        let (dd, pp) = (d.clone(), precious.clone());
        let mut audit = SwapOnAttempt { swap: Box::new(move || { fs::remove_dir_all(&dd).unwrap(); std::os::unix::fs::symlink(&pp, &dd).unwrap(); }) };
        let o = run(&[item], &pol(&home, &apps), false, &never, &mut audit);
        assert_eq!(why(&o, 0), (Status::Refused, Some(Code::Symlink), reason::SYMLINK), "{o:?}");
        assert!(precious.join("keep").exists());
    }

    #[test]
    fn a_folder_rewritten_between_the_check_and_the_move_is_refused_by_the_second_time_check() {
        let (home, apps) = (temp("toctou-mtime"), temp("toctou-mtime-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        let dd = d.clone();
        // Adding an entry changes the folder's own modification time.
        let mut audit = SwapOnAttempt { swap: Box::new(move || fs::write(dd.join("late"), "x").unwrap()) };
        let o = run(&[item], &pol(&home, &apps), false, &never, &mut audit);
        assert_eq!(why(&o, 0).1, Some(Code::Changed), "{o:?}");
        assert!(d.exists());
    }

    #[test]
    fn a_symlinked_folder_above_the_item_is_refused() {
        let (home, apps, elsewhere) = (temp("above"), temp("above-apps"), temp("above-else"));
        let real = cache_dir(&elsewhere, "app");
        fs::create_dir_all(home.join("Library")).unwrap();
        std::os::unix::fs::symlink(elsewhere.join("Library/Caches"), home.join("Library/Caches")).unwrap();
        let item = scanned(&home.join("Library/Caches/app"), Label::Safe);
        let o = go(&[item], &pol(&home, &apps), false, &never);
        assert_eq!(why(&o, 0).1, Some(Code::Outside));
        assert!(real.exists());
    }

    #[test]
    fn a_path_in_a_different_letter_case_is_refused() {
        let (home, apps) = (temp("case"), temp("case-apps"));
        let d = cache_dir(&home, "App");
        let wrong = home.join("library/caches/app");
        // On a case-sensitive volume the other spelling does not exist, which is refused as gone.
        let exists = wrong.exists();
        let mut item = scanned(&d, Label::Safe);
        item.path = wrong.display().to_string();
        let o = go(&[item], &pol(&home, &apps), false, &never);
        assert_eq!(why(&o, 0).1, Some(if exists { Code::Outside } else { Code::Gone }), "{o:?}");
        assert!(d.exists());
    }

    #[test]
    fn an_item_that_changed_after_the_scan_is_refused_even_within_the_same_second() {
        let (home, apps) = (temp("changed"), temp("changed-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        // Rewriting a file leaves the folder's own time alone; the size check catches it.
        fs::write(d.join("blob"), vec![2u8; 400_000]).unwrap();
        let o = go(&[item], &pol(&home, &apps), false, &never);
        assert_eq!(why(&o, 0), (Status::Refused, Some(Code::Changed), reason::CHANGED));
        assert!(d.exists());
    }

    #[test]
    fn a_few_bytes_appended_inside_the_same_block_are_noticed() {
        let (home, apps) = (temp("append"), temp("append-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        let mut f = fs::OpenOptions::new().append(true).open(d.join("blob")).unwrap();
        std::io::Write::write_all(&mut f, b"more").unwrap();
        let o = go(&[item], &pol(&home, &apps), false, &never);
        assert_eq!(why(&o, 0).1, Some(Code::Changed));
    }

    #[test]
    fn an_item_with_a_new_modification_time_is_refused() {
        let (home, apps) = (temp("mtime"), temp("mtime-apps"));
        let d = cache_dir(&home, "app");
        let mut item = scanned(&d, Label::Safe);
        item.modified_ns = item.modified_ns.map(|n| n - 1);
        let o = go(&[item], &pol(&home, &apps), false, &never);
        assert_eq!(why(&o, 0).1, Some(Code::Changed));
    }

    #[test]
    fn a_yours_item_needs_the_second_confirmation() {
        let (home, apps) = (temp("yours"), temp("yours-apps"));
        let d = cache_dir(&home, "mine");
        let item = scanned(&d, Label::Yours);
        let o = go(&[item.clone()], &pol(&home, &apps), false, &never);
        assert_eq!(why(&o, 0), (Status::Refused, Some(Code::NeedsYoursConfirmation), reason::YOURS));
        let log = RefCell::new(vec![]);
        let confirmed = Policy { confirm_yours: true, ..pol(&home, &apps) };
        let o = go(&[item], &confirmed, false, &recording(&log));
        assert_eq!((o.counts().moved, o.refused().len()), (1, 0));
    }

    #[test]
    fn an_item_that_was_not_fully_measured_is_refused() {
        let (home, apps) = (temp("partial"), temp("partial-apps"));
        let d = cache_dir(&home, "app");
        let mut item = scanned(&d, Label::Safe);
        item.complete = false;
        assert_eq!(why(&go(&[item.clone()], &pol(&home, &apps), false, &never), 0).1, Some(Code::Unmeasured));
        item.complete = true;
        item.size = Reading::Unavailable("permission denied");
        assert_eq!(why(&go(&[item], &pol(&home, &apps), false, &never), 0).1, Some(Code::Unmeasured));
    }

    #[test]
    fn a_listed_path_outside_home_and_applications_is_refused() {
        let (home, apps) = (temp("outside"), temp("outside-apps"));
        let other = temp("outside-other");
        let f = other.join("file");
        fs::write(&f, "x").unwrap();
        let confirmed = Policy { confirm_yours: true, ..pol(&home, &apps) };
        for p in [f.clone(), PathBuf::from("/etc/hosts"), home.clone(), apps.clone()] {
            let o = go(&[scanned(&p, Label::Safe)], &confirmed, false, &never);
            assert_eq!(why(&o, 0).1, Some(Code::Outside), "{p:?}");
        }
        assert!(f.exists());
    }

    #[test]
    fn an_item_under_applications_may_move() {
        let (home, apps) = (temp("apps-home"), temp("apps"));
        let app = apps.join("Old.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(app.join("Contents/x"), vec![0u8; 5000]).unwrap();
        let log = RefCell::new(vec![]);
        let o = go(&[scanned(&app, Label::Review)], &pol(&home, &apps), false, &recording(&log));
        assert_eq!(o.counts().moved, 1, "{o:?}");
    }

    #[test]
    fn an_item_owned_by_someone_else_is_refused_up_front_with_a_reason() {
        let (home, apps) = (temp("owner"), temp("owner-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        // The same item, judged by a user it does not belong to (root-owned apps look like this).
        let stranger = Policy { uid: me().wrapping_add(1), ..pol(&home, &apps) };
        let o = go(&[item], &stranger, false, &never);
        assert_eq!(why(&o, 0), (Status::Refused, Some(Code::NotOwned), reason::NOT_OWNED));
        assert!(d.exists());
    }

    #[test]
    fn a_pre_check_refusal_does_not_halt_the_batch() {
        let (home, apps) = (temp("mixed"), temp("mixed-apps"));
        let a = scanned(&cache_dir(&home, "a"), Label::Yours);
        let b = scanned(&cache_dir(&home, "b"), Label::Safe);
        let c = scanned(&cache_dir(&home, "c"), Label::Safe);
        let log = RefCell::new(vec![]);
        let o = go(&[a, b, c], &pol(&home, &apps), false, &recording(&log));
        assert_eq!((o.counts().moved, o.counts().refused, o.counts().failed, o.counts().not_attempted), (2, 1, 0, 0));
        assert!(!o.ok());
    }

    #[test]
    fn an_item_that_is_gone_is_reported() {
        let (home, apps) = (temp("gone"), temp("gone-apps"));
        let d = cache_dir(&home, "app");
        let item = scanned(&d, Label::Safe);
        fs::remove_dir_all(&d).unwrap();
        assert_eq!(why(&go(&[item], &pol(&home, &apps), false, &never), 0), (Status::Refused, Some(Code::Gone), reason::GONE));
    }

    #[test]
    fn a_failed_move_stops_the_batch_and_marks_the_failed_item() {
        let (home, apps) = (temp("half"), temp("half-apps"));
        let items: Vec<Item> = ["a", "b", "c"].iter().map(|n| scanned(&cache_dir(&home, n), Label::Safe)).collect();
        let seen = RefCell::new(0);
        let mover = |p: &Path| { *seen.borrow_mut() += 1; if *seen.borrow() == 2 { Err(MoveError::NotPermitted("Operation not permitted".to_string())) } else { fs::remove_dir_all(p).map_err(|e| MoveError::Other(e.to_string())) } };
        let o = go(&items, &pol(&home, &apps), false, &mover);
        let statuses: Vec<_> = o.items.iter().map(|i| i.status).collect();
        assert_eq!(statuses, [Status::Moved, Status::Failed, Status::NotAttempted]);
        assert_eq!((o.items[1].code, o.items[1].reason.as_deref()), (Some(Code::MoveNotPermitted), Some("Operation not permitted")));
        assert_eq!((o.items[2].code, o.items[2].reason.as_deref()), (Some(Code::EarlierMoveFailed), Some(reason::NOT_ATTEMPTED)));
        assert_eq!(*seen.borrow(), 2);
        assert!(Path::new(&items[2].path).exists());
    }

    #[test]
    fn an_exhausted_time_budget_leaves_the_rest_not_attempted_and_unmoved() {
        let (home, apps) = (temp("budget"), temp("budget-apps"));
        let items: Vec<Item> = ["a", "b"].iter().map(|n| scanned(&cache_dir(&home, n), Label::Safe)).collect();
        let late = Policy { deadline: Instant::now(), ..pol(&home, &apps) };
        let o = go(&items, &late, false, &never);
        assert!(o.items.iter().all(|i| i.status == Status::NotAttempted && i.code == Some(Code::TimeBudget)), "{o:?}");
        assert!(items.iter().all(|i| Path::new(&i.path).exists()));
    }

    /// Records the order of audit calls against moves.
    #[derive(Default)]
    struct Log(Vec<String>);
    struct Ordered<'a> { log: &'a RefCell<Log>, fail_attempt_for: Option<String> }
    impl Audit for Ordered<'_> {
        fn attempting(&mut self, i: &Item) -> Result<(), String> {
            if self.fail_attempt_for.as_deref() == Some(i.id.as_str()) { return Err("locked".into()); }
            self.log.borrow_mut().0.push(format!("attempting {}", i.id)); Ok(())
        }
        fn finished(&mut self, r: &ItemResult) -> Result<(), String> { self.log.borrow_mut().0.push(format!("finished {} {}", r.id, r.status.as_str())); Ok(()) }
    }

    #[test]
    fn each_item_is_audited_before_it_moves_and_finished_after() {
        let (home, apps) = (temp("order"), temp("order-apps"));
        let items: Vec<Item> = ["a", "b"].iter().map(|n| scanned(&cache_dir(&home, n), Label::Safe)).collect();
        let log = RefCell::new(Log::default());
        let mover = |p: &Path| { log.borrow_mut().0.push(format!("move {}", p.file_name().unwrap().to_string_lossy())); fs::remove_dir_all(p).map_err(|e| MoveError::Other(e.to_string())) };
        let o = run(&items, &pol(&home, &apps), false, &mover, &mut Ordered { log: &log, fail_attempt_for: None });
        assert_eq!(o.counts().moved, 2);
        let (a, b) = (&items[0].id, &items[1].id);
        assert_eq!(log.borrow().0, [format!("attempting {a}"), "move a".to_string(), format!("finished {a} moved"), format!("attempting {b}"), "move b".to_string(), format!("finished {b} moved")]);
    }

    #[test]
    fn an_item_whose_audit_row_cannot_be_written_is_not_moved_and_stops_the_request() {
        let (home, apps) = (temp("auditfail"), temp("auditfail-apps"));
        let items: Vec<Item> = ["a", "b", "c"].iter().map(|n| scanned(&cache_dir(&home, n), Label::Safe)).collect();
        let log = RefCell::new(Log::default());
        let moves = RefCell::new(vec![]);
        let o = run(&items, &pol(&home, &apps), false, &recording(&moves), &mut Ordered { log: &log, fail_attempt_for: Some(items[1].id.clone()) });
        let statuses: Vec<_> = o.items.iter().map(|i| i.status).collect();
        assert_eq!(statuses, [Status::Moved, Status::NotAttempted, Status::NotAttempted]);
        assert_eq!(o.items[1].code, Some(Code::AuditFailed));
        assert_eq!(moves.borrow().len(), 1);
        assert!(Path::new(&items[1].path).exists() && Path::new(&items[2].path).exists());
    }

    #[test]
    fn a_dry_run_never_calls_the_audit() {
        let (home, apps) = (temp("dry-audit"), temp("dry-audit-apps"));
        let item = scanned(&cache_dir(&home, "a"), Label::Safe);
        let log = RefCell::new(Log::default());
        let o = run(&[item], &pol(&home, &apps), true, &never, &mut Ordered { log: &log, fail_attempt_for: None });
        assert_eq!(o.counts().would_move, 1);
        assert!(log.borrow().0.is_empty());
    }

    /// How long a killed command gets to be reaped before it is given up on. A process stuck in the kernel (an
    /// `hdiutil` waiting on `diskimages-helper`) can ignore SIGKILL, and an unbounded `wait()` on it hangs the whole run.
    const REAP_SECS: u64 = 5;

    /// Runs a command and kills it after `secs`. `Ok` only for a clean exit; otherwise the error names the command and how long it ran.
    /// A child that survives the kill is abandoned after `REAP_SECS` and left until the test binary exits.
    /// Nothing here waits without a limit: after the kill it polls for `REAP_SECS` and then abandons the child.
    pub(crate) fn run_within(cmd: &mut std::process::Command, secs: u64) -> Result<(), String> {
        let name = format!("{cmd:?}");
        let started = Instant::now();
        let mut child = cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map_err(|e| format!("could not start {name}: {e}"))?;
        let until = started + Duration::from_secs(secs);
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(status)) => return Err(format!("{name} exited with {status} after {:.1}s", started.elapsed().as_secs_f32())),
                Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(50)),
                Ok(None) => {
                    let _ = child.kill();
                    let reap_until = Instant::now() + Duration::from_secs(REAP_SECS);
                    while Instant::now() < reap_until {
                        if let Ok(Some(_)) = child.try_wait() { return Err(format!("{name} timed out and was killed after {:.1}s (limit {secs}s)", started.elapsed().as_secs_f32())); }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    return Err(format!("{name} timed out after {:.1}s (limit {secs}s) and did not exit {REAP_SECS}s after SIGKILL; abandoned", started.elapsed().as_secs_f32()));
                }
                Err(e) => return Err(format!("{name}: waiting failed after {:.1}s: {e}", started.elapsed().as_secs_f32())),
            }
        }
    }

    /// Set this to `1` to run the hdiutil mount tests where they would otherwise be skipped (CI).
    pub(crate) const RUN_MOUNT_TESTS: &str = "ML_RUN_MOUNT_TESTS";

    /// True where the hdiutil tests must not run: GitHub Actions or another CI (`GITHUB_ACTIONS` or `CI` set), unless
    /// `ML_RUN_MOUNT_TESTS=1`. A hosted macOS runner has frozen for good during `hdiutil detach -force`, and a frozen runner
    /// uploads no log. So the mount-crossing refusals are verified on a developer's machine only. The notice goes straight to
    /// stderr, which the test harness does not capture, so it is visible in the CI log.
    pub(crate) fn skip_mount_tests(test: &str) -> bool {
        let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty() && v != "0" && v != "false");
        if std::env::var(RUN_MOUNT_TESTS).as_deref() == Ok("1") || !(set("GITHUB_ACTIONS") || set("CI")) { return false; }
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "\n*** SKIPPED IN CI: {test}: hdiutil can freeze the runner; run locally with `{RUN_MOUNT_TESTS}=1 cargo test mount` (the mount refusals are verified locally only) ***\n");
        true
    }

    /// True when `p` is on a different device than its parent, that is, something is mounted there.
    fn is_mounted(p: &Path) -> bool {
        match (fs::metadata(p), p.parent().and_then(|q| fs::metadata(q).ok())) { (Ok(a), Some(b)) => a.dev() != b.dev(), _ => false }
    }

    /// A disk image attached at a mount point in a temp folder. Every hdiutil call has a time limit. Dropping it detaches the
    /// volume with `-force`, even when the test panics, then deletes the `.dmg`; if the detach fails the folders are left alone
    /// rather than walked into.
    pub(crate) struct Mounted { mnt: PathBuf, root: PathBuf, img: PathBuf }
    impl Mounted {
        /// None where a disk image cannot be attached (a sandbox, a headless runner), with the reason printed.
        pub(crate) fn attach(mnt: &Path, scratch: &Path, root: &Path) -> Option<Mounted> {
            use std::process::Command;
            let img = scratch.join("v.dmg");
            let guard = Mounted { mnt: mnt.to_path_buf(), root: root.to_path_buf(), img: img.clone() };
            let made = run_within(Command::new("/usr/bin/hdiutil").args(["create", "-size", "8m", "-fs", "HFS+", "-volname", "mltest", "-quiet"]).arg(&img), 30);
            let attached = made.and_then(|()| run_within(Command::new("/usr/bin/hdiutil").args(["attach", "-nobrowse", "-quiet", "-mountpoint"]).arg(mnt).arg(&img), 30));
            match attached {
                Ok(()) => Some(guard),
                Err(why) => {
                    eprintln!("skipped: hdiutil could not attach a disk image here: {why}");
                    // A timed-out attach may still have mounted: only then does the guard need to detach.
                    if !is_mounted(mnt) { let _ = fs::remove_file(&img); std::mem::forget(guard); }
                    None
                }
            }
        }
    }
    impl Drop for Mounted {
        fn drop(&mut self) {
            match run_within(std::process::Command::new("/usr/bin/hdiutil").args(["detach", "-force", "-quiet"]).arg(&self.mnt), 30) {
                Ok(()) => { let _ = fs::remove_file(&self.img); }
                // Already unmounted counts as detached.
                Err(_) if !is_mounted(&self.mnt) => { let _ = fs::remove_file(&self.img); }
                Err(why) => {
                    eprintln!("could not detach the test volume at {:?}; leaving its folders and image in place: {why}", self.mnt);
                    keep(&self.root);
                    keep(&self.img);
                }
            }
        }
    }

    #[test]
    fn run_within_kills_a_command_that_overruns_and_names_it_in_the_error() {
        let t = Instant::now();
        let err = run_within(std::process::Command::new("/bin/sleep").arg("30"), 1).unwrap_err();
        assert!(t.elapsed() < Duration::from_secs(10), "it returned promptly: {:?}", t.elapsed());
        assert!(err.contains("sleep") && err.contains("timed out") && err.contains("limit 1s"), "{err}");
        assert!(run_within(&mut std::process::Command::new("/usr/bin/true"), 5).is_ok());
        assert!(run_within(&mut std::process::Command::new("/usr/bin/false"), 5).unwrap_err().contains("exited with"));
    }

    // The two mount tests run `hdiutil`, which has frozen a hosted macOS runner for good (no log was uploaded). They are skipped
    // when `GITHUB_ACTIONS` or `CI` is set, so the mount-crossing refusal is verified on a developer's machine only. To run them
    // anywhere: `ML_RUN_MOUNT_TESTS=1 cargo test mount`.
    #[test]
    fn an_item_with_a_volume_mounted_inside_it_is_refused() {
        if skip_mount_tests("an_item_with_a_volume_mounted_inside_it_is_refused") { return; }
        let (home, apps, scratch) = (temp("mount"), temp("mount-apps"), temp("mount-img"));
        let d = cache_dir(&home, "app");
        let mnt = d.join("mnt");
        fs::create_dir_all(&mnt).unwrap();
        let Some(_volume) = Mounted::attach(&mnt, &scratch, &home) else { return };
        let item = scanned(&d, Label::Safe);
        let o = go(&[item], &pol(&home, &apps), false, &never);
        assert_eq!(why(&o, 0), (Status::Refused, Some(Code::MountInside), reason::MOUNT_INSIDE), "{o:?}");
        assert!(d.exists());
    }

    #[test]
    fn an_item_that_is_itself_a_mount_point_is_refused() {
        if skip_mount_tests("an_item_that_is_itself_a_mount_point_is_refused") { return; }
        let (home, apps, scratch) = (temp("mountpoint"), temp("mountpoint-apps"), temp("mountpoint-img"));
        let d = home.join("Library/Caches/app");
        fs::create_dir_all(&d).unwrap();
        let Some(_volume) = Mounted::attach(&d, &scratch, &home) else { return };
        let item = scanned(&d, Label::Safe);
        let o = go(&[item], &pol(&home, &apps), false, &never);
        assert_eq!(why(&o, 0), (Status::Refused, Some(Code::MountInside), reason::MOUNT_POINT), "{o:?}");
    }

    #[test]
    fn an_item_not_in_the_preview_is_refused_even_if_it_would_now_pass() {
        let (home, apps) = (temp("preview-set"), temp("preview-set-apps"));
        let items: Vec<Item> = ["a", "b"].iter().map(|n| scanned(&cache_dir(&home, n), Label::Safe)).collect();
        let only_a = vec![items[0].id.clone()];
        let p = Policy { previewed: Some(&only_a), ..pol(&home, &apps) };
        let log = RefCell::new(vec![]);
        let o = go(&items, &p, false, &recording(&log));
        assert_eq!((o.items[0].status, o.items[1].status, o.items[1].code), (Status::Moved, Status::Refused, Some(Code::NotPreviewed)));
        assert!(Path::new(&items[1].path).exists());
    }

    #[test]
    fn ids_must_be_sixteen_hex_digits_unique_and_few() {
        assert_eq!(parse_ids("0123456789abcdef,0123456789abcdef").unwrap().len(), 1);
        for bad in ["", ",", "xyz", "0123456789abcde", "0123456789abcdeg", "../../etc/passwd", "0123456789abcdef/0123"] { assert!(parse_ids(bad).is_err(), "{bad}"); }
        let many = (0..=MAX_IDS).map(|i| format!("{i:016x}")).collect::<Vec<_>>().join(",");
        assert!(parse_ids(&many).is_err());
    }

    #[test]
    fn a_crafted_id_cannot_reach_a_path_the_scan_did_not_list() {
        let home = temp("craft");
        let d = cache_dir(&home, "app");
        let mut s = Storage::fake(1);
        s.items.push(scanned(&d, Label::Safe));
        let cache = Cache::with_result(s);
        // The id of a path nobody scanned, built with the same hash.
        for path in ["/etc", "/etc/hosts", "/Users/someone/Documents"] {
            assert!(cache.find(&[item_id(path)]).unwrap_err().contains("not in the latest scan"), "{path}");
        }
        assert_eq!(cache.find(&[item_id(&d.display().to_string())]).unwrap().len(), 1);
        assert!(Cache::default().find(&["0123456789abcdef".into()]).is_err(), "no scan yet");
    }
}
