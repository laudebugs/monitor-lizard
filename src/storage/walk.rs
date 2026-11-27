//! The file-system side of the scan: sizing one location with `lstat`, pacing so the scan stays a
//! small share of one core, the search for `node_modules` folders and the idle test for a project.
use super::reason;
use std::collections::HashSet;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const IDLE_SECS: u64 = 90 * 24 * 3600;
const NODE_MODULES_DEPTH: usize = 5;
pub const MAX_NODE_MODULES: usize = 300;
/// Folders where projects usually live, lowercase. Searched deeply; the home folder itself and its
/// direct children are searched one level only.
const PROJECT_ROOTS: &[&str] = &["developer", "projects", "code", "src", "dev", "work", "git", "repos", "workspace"];
const LOCKFILES: &[&str] = &["package-lock.json", "yarn.lock", "pnpm-lock.yaml", "bun.lockb"];

#[derive(Clone, Copy, Debug)]
pub struct Limits { pub deadline: Instant, pub max_entries: u64 }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stop { EntryCap, Time }

#[derive(Default, Debug)]
pub struct Walked {
    /// `st_blocks * 512`: what the files take on disk, so a sparse file is not over-counted.
    pub allocated: u64,
    pub apparent: u64,
    pub entries: u64,
    /// Subfolders or entries that could not be read; their size is missing from the total.
    pub unreadable: u32,
    /// Subfolders on another device (a mount point) that the walk did not enter, so their content is
    /// not in the total.
    pub crossed: u32,
    pub stop: Option<Stop>,
}

pub fn io_reason(e: &io::Error) -> &'static str {
    // Without Full Disk Access macOS answers EPERM (not EACCES) for protected folders.
    match e.raw_os_error() {
        Some(libc::EACCES) | Some(libc::EPERM) => reason::PERMISSION_DENIED,
        _ => reason::UNREADABLE,
    }
}

/// Sizes one location. `None` when it does not exist (the caller omits it, never shows 0).
pub fn walk(root: &Path, lim: Limits) -> Option<Result<Walked, &'static str>> {
    let meta = match std::fs::symlink_metadata(root) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => return Some(Err(io_reason(&e))),
    };
    let mut w = Walked::default();
    count(&mut w, &meta);
    if !meta.is_dir() { return Some(Ok(w)); }
    let dev = meta.dev();
    // An unreadable top folder must read as unavailable, not as an empty folder.
    if let Err(e) = std::fs::read_dir(root) { return Some(Err(io_reason(&e))); }
    let mut pending = vec![root.to_path_buf()];
    let mut linked: HashSet<(u64, u64)> = HashSet::new();
    let mut since_clock = 0u32;
    while let Some(dir) = pending.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { w.unreadable += 1; continue };
        for entry in rd {
            let Ok(entry) = entry else { w.unreadable += 1; continue };
            w.entries += 1;
            if w.entries > lim.max_entries { w.stop = Some(Stop::EntryCap); return Some(Ok(w)); }
            since_clock += 1;
            if since_clock >= 16 {
                since_clock = 0;
                if Instant::now() >= lim.deadline { w.stop = Some(Stop::Time); return Some(Ok(w)); }
                pace();
            }
            // `DirEntry::metadata` is an `lstat`: a symlink is sized as itself and never followed.
            let Ok(m) = entry.metadata() else { w.unreadable += 1; continue };
            if m.is_dir() {
                if m.dev() != dev { w.crossed += 1; continue; }
                count(&mut w, &m);
                pending.push(entry.path());
            } else if m.nlink() > 1 && !linked.insert((m.dev(), m.ino())) {
                // A second name for a file already counted.
            } else {
                count(&mut w, &m);
            }
        }
    }
    if Instant::now() >= lim.deadline { w.stop = Some(Stop::Time); }
    Some(Ok(w))
}

fn count(w: &mut Walked, m: &std::fs::Metadata) {
    w.allocated += m.blocks() * 512;
    if m.is_file() { w.apparent += m.len(); }
}

/// A rough size for ordering only: the allocated bytes of a folder's direct entries.
pub fn one_level_bytes(dir: &Path, deadline: Instant) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    let mut total = 0;
    for (i, e) in rd.flatten().enumerate() {
        if i % 64 == 0 {
            if Instant::now() >= deadline { break; }
            pace();
        }
        total += e.metadata().map(|m| m.blocks() * 512).unwrap_or(0);
    }
    total
}

// Pacing. Without a limit a scan of a developer home used about 45% of one core, which would make
// the monitor its own top consumer.

/// Share of one core the scan may use.
const DUTY: f64 = 0.2;
/// Rests shorter than this are skipped: a short sleep on a low-priority thread overshoots a lot.
const MIN_REST: Duration = Duration::from_millis(20);
const MAX_REST: Duration = Duration::from_millis(250);

struct Pacer { start: Instant, cpu_start: Duration }

thread_local!(static PACER: std::cell::RefCell<Option<Pacer>> = const { std::cell::RefCell::new(None) });

pub fn pace_reset() { PACER.with(|p| *p.borrow_mut() = None); }

/// CPU time this thread has used. Wall time would count disk waits as work and rest too much.
fn thread_cpu() -> Duration {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid out-pointer for the call; on failure it stays zero and pacing does nothing.
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    Duration::new(ts.tv_sec.max(0) as u64, ts.tv_nsec.clamp(0, 999_999_999) as u32)
}

/// Called every few entries: rests when the scan has used more than `DUTY` of one core since it began.
/// The rest is computed from what has really elapsed, so a sleep that overshoots shortens the next one.
pub fn pace() {
    PACER.with(|cell| {
        let mut slot = cell.borrow_mut();
        let p = slot.get_or_insert_with(|| Pacer { start: Instant::now(), cpu_start: thread_cpu() });
        let used = thread_cpu().saturating_sub(p.cpu_start);
        let wanted = used.div_f64(DUTY).saturating_sub(p.start.elapsed());
        if wanted >= MIN_REST { std::thread::sleep(wanted.min(MAX_REST)); }
    });
}

// Finding projects.

fn skipped_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    name.starts_with('.') || lower == "library" || lower == "node_modules" || super::rules::USER_CONTENT.contains(&lower.as_str())
}

/// The folders searched for `node_modules`, with how deep. User-content folders and `~/Library` are
/// never entered: nothing in them can be `safe`, and reading them can raise a macOS access prompt or
/// touch cloud placeholders.
fn project_roots(home: &Path) -> Vec<(PathBuf, usize)> {
    let mut roots = vec![(home.to_path_buf(), 1)];
    let Ok(rd) = std::fs::read_dir(home) else { return roots };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if skipped_name(&name) || !e.file_type().is_ok_and(|t| t.is_dir()) { continue; }
        if PROJECT_ROOTS.contains(&name.to_lowercase().as_str()) { roots.push((e.path(), NODE_MODULES_DEPTH)); }
        // A project folder one level down, such as `~/work/git`.
        if let Ok(inner) = std::fs::read_dir(e.path()) {
            for i in inner.flatten() {
                let n = i.file_name().to_string_lossy().into_owned();
                if PROJECT_ROOTS.contains(&n.to_lowercase().as_str()) && !skipped_name(&n) && i.file_type().is_ok_and(|t| t.is_dir()) { roots.push((i.path(), NODE_MODULES_DEPTH)); }
            }
        }
    }
    roots
}

/// `node_modules` folders in the project roots, each with whether its project is idle, and whether the
/// search was cut short (by the entry cap, the deadline or `MAX_NODE_MODULES`).
pub fn find_node_modules(home: &Path, lim: Limits, now: u64) -> (Vec<(PathBuf, bool)>, bool) {
    let mut found: Vec<(PathBuf, bool)> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let (mut visited, mut cut) = (0u64, false);
    for (root, max_depth) in project_roots(home) {
        let mut pending = vec![(root, 0usize)];
        while let Some((dir, depth)) = pending.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for entry in rd.flatten() {
                visited += 1;
                if visited > lim.max_entries || (visited % 16 == 0 && Instant::now() >= lim.deadline) { return (found, true); }
                if visited % 16 == 0 { pace(); }
                if !entry.file_type().is_ok_and(|t| t.is_dir()) { continue; }
                let name = entry.file_name().to_string_lossy().into_owned();
                if name == "node_modules" {
                    if found.len() >= MAX_NODE_MODULES { cut = true; continue; }
                    if seen.insert(entry.path()) { found.push((entry.path(), project_idle(home, &dir, &entry.path(), now))); }
                } else if depth < max_depth && !skipped_name(&name) {
                    pending.push((entry.path(), depth + 1));
                }
            }
        }
    }
    (found, cut)
}

fn mtime(p: &Path) -> Option<u64> { std::fs::symlink_metadata(p).ok().map(|m| m.mtime().max(0) as u64) }

/// A project is idle only when every signal says so: a lockfile beside `package.json` (without one a
/// reinstall may differ or fail), the nearest git index above it (up to the home folder) older than
/// 90 days, and nothing among the project's top-level entries, `node_modules` included, newer than
/// 90 days. A missing signal means not idle. A monorepo package has no lockfile of its own, so it is
/// never idle: that is deliberate. The check reads folder times, so an in-place edit deep inside
/// `src/` that changes no top-level time and no git index goes unseen.
pub fn project_idle(home: &Path, project: &Path, modules: &Path, now: u64) -> bool {
    let old = |t: u64| now.saturating_sub(t) >= IDLE_SECS;
    if !project.join("package.json").is_file() || !LOCKFILES.iter().any(|l| project.join(l).is_file()) { return false; }
    // The first `.git` going up is the project's repository. If its index is missing or unreadable the
    // project is not idle: an outer repository's index says nothing about it.
    let repo = project.ancestors().take_while(|a| a.starts_with(home)).find(|a| std::fs::symlink_metadata(a.join(".git")).is_ok());
    let Some(git_index) = repo.and_then(|r| mtime(&r.join(".git/index"))) else { return false };
    let Ok(rd) = std::fs::read_dir(project) else { return false };
    let top_level = rd.flatten().filter(|e| e.file_name() != "node_modules").map(|e| mtime(&e.path()));
    let Some(newest) = top_level.chain([mtime(project), mtime(modules)]).collect::<Option<Vec<_>>>().and_then(|v| v.into_iter().max()) else { return false };
    old(git_index) && old(newest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::tests::temp;
    use std::fs;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::process::Command;

    fn lim(max_entries: u64) -> Limits { Limits { deadline: Instant::now() + Duration::from_secs(30), max_entries } }

    fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() }

    fn is_root() -> bool {
        // SAFETY: `geteuid` takes no arguments and cannot fail.
        unsafe { libc::geteuid() == 0 }
    }

    #[test]
    fn allocated_size_is_blocks_not_apparent_size_for_a_sparse_file() {
        let d = temp("sparse");
        let f = fs::File::create(d.join("Docker.raw")).unwrap();
        f.set_len(200 * 1024 * 1024).unwrap();
        let w = walk(&d, lim(1000)).unwrap().unwrap();
        assert_eq!(w.apparent, 200 * 1024 * 1024);
        assert!(w.allocated < 10 * 1024 * 1024, "allocated {} should be far below the apparent size", w.allocated);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_symlink_loop_terminates_and_a_link_to_a_big_folder_is_not_followed() {
        let d = temp("loop");
        let outside = temp("outside");
        fs::write(outside.join("big"), vec![1u8; 1024 * 1024]).unwrap();
        fs::create_dir(d.join("a")).unwrap();
        symlink(&d, d.join("a/up")).unwrap();
        symlink(d.join("a"), d.join("a/self")).unwrap();
        symlink(&outside, d.join("to-outside")).unwrap();
        let w = walk(&d, lim(1000)).unwrap().unwrap();
        assert_eq!(w.entries, 4, "a, up, self and to-outside, each once");
        assert!(w.allocated < 512 * 1024, "the 1 MB file behind the link must not be counted: {}", w.allocated);
        assert_eq!(w.stop, None);
        fs::remove_dir_all(&d).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn a_hard_linked_file_is_counted_once() {
        let d = temp("hardlink");
        fs::write(d.join("one"), vec![1u8; 256 * 1024]).unwrap();
        fs::hard_link(d.join("one"), d.join("two")).unwrap();
        let w = walk(&d, lim(1000)).unwrap().unwrap();
        assert!(w.allocated < 400 * 1024, "{}", w.allocated);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_time_budget_stops_the_walk_and_says_so() {
        let d = temp("budget");
        for i in 0..200 { fs::write(d.join(format!("f{i}")), b"x").unwrap(); }
        let past = Limits { deadline: Instant::now() - Duration::from_secs(1), max_entries: u64::MAX };
        let w = walk(&d, past).unwrap().unwrap();
        assert_eq!(w.stop, Some(Stop::Time));
        assert!(w.entries < 200, "stopped early: {}", w.entries);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_entry_cap_stops_the_walk() {
        let d = temp("cap");
        for i in 0..50 { fs::write(d.join(format!("f{i}")), b"x").unwrap(); }
        assert_eq!(walk(&d, lim(10)).unwrap().unwrap().stop, Some(Stop::EntryCap));
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_denied_folder_is_unavailable_not_zero() {
        if is_root() { eprintln!("skipped: running as root, where mode 000 does not deny access"); return; }
        let d = temp("denied");
        let locked = d.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("f"), b"data").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let r = walk(&locked, lim(100)).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(r.unwrap_err(), reason::PERMISSION_DENIED);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_denied_subfolder_is_counted_and_the_rest_is_still_sized() {
        if is_root() { eprintln!("skipped: running as root, where mode 000 does not deny access"); return; }
        let d = temp("denied-sub");
        fs::write(d.join("ok"), vec![1u8; 64 * 1024]).unwrap();
        let locked = d.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let w = walk(&d, lim(100)).unwrap().unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(w.unreadable, 1);
        assert!(w.allocated >= 64 * 1024);
        fs::remove_dir_all(&d).unwrap();
    }

    /// Removes the temp folder and detaches the image even when an assert fails.
    struct Mounted { dir: PathBuf, mount: PathBuf }
    impl Drop for Mounted {
        fn drop(&mut self) {
            let _ = Command::new("/usr/bin/hdiutil").args(["detach", "-force"]).arg(&self.mount).output();
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    /// Mounts a tiny disk image inside a folder and checks the walk stays on the folder's own volume.
    /// Ignored by default because mounting needs a session that allows `hdiutil` (hosted CI runners
    /// may not); run it with `cargo test -- --ignored`. It fails, not skips, if the mount fails.
    #[test]
    #[ignore = "mounts a disk image with hdiutil; run with --ignored"]
    fn a_walk_never_crosses_into_another_volume() {
        let dir = temp("device");
        let mount = dir.join("root/mnt");
        let guard = Mounted { dir: dir.clone(), mount: mount.clone() };
        fs::create_dir_all(&mount).unwrap();
        let image = dir.join("v.dmg");
        let run = |c: &mut Command| { let o = c.output().expect("hdiutil runs"); assert!(o.status.success(), "hdiutil failed: {}", String::from_utf8_lossy(&o.stderr)); };
        run(Command::new("/usr/bin/hdiutil").args(["create", "-size", "4m", "-fs", "HFS+", "-volname", "t", "-ov"]).arg(&image));
        run(Command::new("/usr/bin/hdiutil").args(["attach", "-nobrowse", "-mountpoint"]).arg(&mount).arg(&image));
        fs::write(mount.join("big"), vec![1u8; 1024 * 1024]).unwrap();
        fs::write(dir.join("root/own"), b"x").unwrap();
        let w = walk(&dir.join("root"), lim(1000)).unwrap().unwrap();
        assert_eq!(w.entries, 2, "`own` and the mount point, whose contents are not visited");
        assert!(w.allocated < 512 * 1024, "{}", w.allocated);
        drop(guard);
    }

    /// Sets the modification time of every file and folder under `dir` (and `dir`) to `days` ago.
    fn age(dir: &Path, days: u64) {
        let t = (now() - days * 24 * 3600) as libc::time_t;
        let tv = [libc::timeval { tv_sec: t, tv_usec: 0 }; 2];
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                if e.file_type().unwrap().is_dir() { age(&e.path(), days); } else { set(&e.path(), &tv); }
            }
        }
        set(dir, &tv);
    }

    fn set(p: &Path, tv: &[libc::timeval; 2]) {
        let c = std::ffi::CString::new(p.to_str().unwrap()).unwrap();
        // SAFETY: `c` is a valid NUL-terminated path and `tv` points at two timevals, as `utimes` requires.
        assert_eq!(unsafe { libc::utimes(c.as_ptr(), tv.as_ptr()) }, 0);
    }

    fn project(home: &Path, rel: &str, lockfile: bool) -> (PathBuf, PathBuf) {
        let p = home.join(rel);
        fs::create_dir_all(p.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(p.join("src")).unwrap();
        fs::write(p.join("package.json"), b"{}").unwrap();
        fs::write(p.join("src/index.js"), b"x").unwrap();
        if lockfile { fs::write(p.join("package-lock.json"), b"{}").unwrap(); }
        let m = p.join("node_modules");
        (p, m)
    }

    fn git_at(dir: &Path) { fs::create_dir_all(dir.join(".git")).unwrap(); fs::write(dir.join(".git/index"), b"i").unwrap(); }

    #[test]
    fn a_project_is_idle_only_when_every_signal_is_old() {
        let home = temp("idle");
        let (p, m) = project(&home, "code/app", true);
        git_at(&p);
        assert!(!project_idle(&home, &p, &m, now()), "fresh files");
        age(&p, 120);
        assert!(project_idle(&home, &p, &m, now()), "everything 120 days old");
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_project_with_a_fresh_source_file_is_not_idle() {
        let home = temp("idle-fresh");
        let (p, m) = project(&home, "code/app", true);
        git_at(&p);
        age(&p, 120);
        fs::write(p.join("src/new.js"), b"edit").unwrap();
        // Setting the folder's time back isolates the file: only a top-level entry is fresh here.
        age(&p.join("src"), 0);
        assert!(!project_idle(&home, &p, &m, now()));
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_project_without_a_lockfile_or_a_git_index_is_not_idle() {
        let home = temp("idle-missing");
        let (p, m) = project(&home, "code/nolock", false);
        git_at(&p);
        age(&p, 120);
        assert!(!project_idle(&home, &p, &m, now()), "no lockfile");
        let (q, qm) = project(&home, "code/nogit", true);
        age(&q, 120);
        assert!(!project_idle(&home, &q, &qm, now()), "no git index anywhere up to home");
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn the_first_git_folder_going_up_decides_and_an_outer_repository_is_not_used() {
        let home = temp("idle-nested");
        let (outer, _) = project(&home, "code/outer", true);
        git_at(&outer);
        let (inner, im) = project(&outer, "tools/inner", true);
        // The inner project has a `.git` without an index; the outer repository's old index must not stand in.
        fs::create_dir_all(inner.join(".git")).unwrap();
        age(&outer, 120);
        assert!(!project_idle(&home, &inner, &im, now()));
        fs::write(inner.join(".git/index"), b"i").unwrap();
        age(&inner, 120);
        assert!(project_idle(&home, &inner, &im, now()), "with its own old index it is idle");
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_monorepo_package_is_never_idle_and_an_active_repo_keeps_old_packages_in_use() {
        let home = temp("idle-mono");
        let (root, _) = project(&home, "code/mono", true);
        git_at(&root);
        let (pkg, pm) = project(&root, "packages/x", false);
        age(&root, 120);
        assert!(!project_idle(&home, &pkg, &pm, now()), "a workspace package has no lockfile of its own");
        // Even with a lockfile in the package, a fresh repository index above it keeps it in use.
        fs::write(pkg.join("package-lock.json"), b"{}").unwrap();
        age(&pkg, 120);
        fs::write(root.join(".git/index"), b"new").unwrap();
        assert!(!project_idle(&home, &pkg, &pm, now()), "the nearest git index is fresh");
        fs::remove_dir_all(&home).unwrap();
    }
}
