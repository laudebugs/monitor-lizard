//! Catches privilege-escalation tools (sudo, su) the moment they start and records who launched them.
//! Polls the kernel process table (cheap libproc calls); no entitlements needed. A process that starts
//! and exits entirely between two polls (20 ms) can be missed; Endpoint Security would close that gap.
use libproc::{bsd_info::BSDInfo, proc_pid, processes::{pids_by_type, ProcFilter}};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const WATCHED: [&str; 2] = ["/usr/bin/sudo", "/usr/bin/su"];
const POLL: Duration = Duration::from_millis(20);
/// A new pid can still be a forked shell that hasn't exec'd yet, so re-check fresh pids for a while.
const FRESH: Duration = Duration::from_millis(1500);

/// One process between the person and `sudo`. Rows written before the detail fields existed hold only
/// `pid`, `exe` and `command`; every later field defaults, so they still load (`detail` is then false,
/// which is how the page tells "not recorded" from "could not be read").
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Hop {
    pub pid: u32, pub exe: String, pub command: String,
    /// Start time, seconds since the epoch. None when the kernel would not say.
    #[serde(default)] pub started: Option<u64>,
    /// Controlling terminal such as `ttys003`. None for a process with no terminal, or when unread.
    #[serde(default)] pub tty: Option<String>,
    /// Working directory. None when the process is another user's or had already exited.
    #[serde(default)] pub cwd: Option<String>,
    /// True when this build read the detail fields, so a None above means "not readable", not "not recorded".
    #[serde(default)] pub detail: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SudoEvent { pub ts: u64, pub pid: u32, pub tool: String, pub args: String, pub chain: Vec<Hop> }

fn ppid(pid: u32) -> Option<u32> { proc_pid::pidinfo::<BSDInfo>(pid as i32, 0).ok().map(|b| b.pbi_ppid) }
fn pids() -> Vec<u32> { pids_by_type(ProcFilter::All).unwrap_or_default() }

/// How much of a command line is kept; the page says when a command reached this length.
pub const COMMAND_MAX: usize = 300;

/// `ps -o pid=,tty=,command=` rows: pid, controlling terminal (`??` means none) and the command.
pub fn parse_ps(text: &str) -> HashMap<u32, (Option<String>, String)> {
    text.lines().filter_map(|l| {
        let l = l.trim_start();
        let (p, rest) = l.split_once(char::is_whitespace)?;
        let rest = rest.trim_start();
        let (tty, cmd) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let tty = (!tty.is_empty() && !tty.starts_with('?') && tty != "-").then(|| tty.to_string());
        Some((p.parse().ok()?, (tty, cmd.trim().chars().take(COMMAND_MAX).collect())))
    }).collect()
}

fn ps_rows(pids: &[u32]) -> HashMap<u32, (Option<String>, String)> {
    if pids.is_empty() { return HashMap::new(); }
    let list = pids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let out = Command::new("ps").args(["-ww", "-o", "pid=,tty=,command=", "-p", &list]).output();
    parse_ps(&out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default())
}

fn commands(pids: &[u32]) -> HashMap<u32, String> {
    ps_rows(pids).into_iter().map(|(p, (_, c))| (p, c)).collect()
}

/// Working directory of `pid` (`PROC_PIDVNODEPATHINFO`). The kernel answers for the caller's own processes;
/// for another user's, or one that has exited, it fails and this is None.
pub fn cwd_of(pid: u32) -> Option<String> {
    // SAFETY: the buffer is a plain C struct we own, `proc_pidinfo` writes at most its size, and the
    // path is NUL-terminated inside the fixed array.
    unsafe {
        let mut info: libc::proc_vnodepathinfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
        let n = libc::proc_pidinfo(pid as libc::c_int, libc::PROC_PIDVNODEPATHINFO, 0, &mut info as *mut _ as *mut libc::c_void, size);
        if n < size { return None; }
        let bytes: &[u8] = std::slice::from_raw_parts(info.pvi_cdir.vip_path.as_ptr() as *const u8, 1024);
        let end = bytes.iter().position(|&b| b == 0)?;
        let path = String::from_utf8_lossy(&bytes[..end]).into_owned();
        (!path.is_empty()).then_some(path)
    }
}

fn started_of(pid: u32) -> Option<u64> {
    proc_pid::pidinfo::<BSDInfo>(pid as i32, 0).ok().map(|b| b.pbi_start_tvsec).filter(|&t| t > 0)
}

const INTERPRETER_NAMES: [&str; 17] = ["node", "bun", "deno", "ruby", "perl", "php", "bash", "zsh", "sh", "dash", "fish", "ksh", "tcsh", "csh", "lua", "Rscript", "osascript"];
/// Options of an interpreter that take the next word as their value, so that word is not the script.
const VALUE_OPTIONS: [&str; 14] = ["-r", "--require", "--loader", "--import", "--experimental-loader", "--conditions", "-C", "-W", "-X", "-I", "-o", "-O", "--rcfile", "--init-file"];
/// Options after which the rest of the line is inline code or a module, not a script path.
const INLINE_OPTIONS: [&str; 4] = ["-c", "-e", "-p", "-E"];

fn is_interpreter(name: &str) -> bool {
    INTERPRETER_NAMES.contains(&name)
        || name.strip_prefix("python").is_some_and(|v| v.chars().all(|c| c.is_ascii_digit() || c == '.'))
}

fn basename(p: &str) -> &str { p.rsplit('/').next().unwrap_or(p) }

/// A short name for one hop: the program, and when the program is an interpreter, the script or module
/// it runs (`node script.js`, `bash -c`, `python3 -m http.server`), so `node` alone is never the whole answer.
/// The page has the same function (`hopLabel`); both are checked against `tests/fixtures/hop-labels.json`.
pub fn hop_label(command: &str, exe: &str) -> String {
    let command = command.trim();
    // An app path can hold spaces, which `ps` does not quote: trust the executable path when the command starts with it.
    let (first, rest) = if !exe.is_empty() && command.strip_prefix(exe).is_some_and(|r| r.is_empty() || r.starts_with(' ')) {
        (exe, &command[exe.len()..])
    } else {
        command.split_once(char::is_whitespace).map(|(a, b)| (a, b)).unwrap_or((command, ""))
    };
    let mut words: Vec<&str> = rest.split_whitespace().collect();
    let mut name = basename(first).trim_start_matches('-');
    if name.is_empty() { name = basename(exe); }
    if name.is_empty() { return "?".into(); }
    if name == "env" {
        let skip = words.iter().take_while(|w| w.starts_with('-') || w.contains('=')).count();
        if skip < words.len() { name = basename(words[skip]); words.drain(..=skip); }
    }
    if !is_interpreter(name) { return name.to_string(); }
    let mut it = words.into_iter();
    while let Some(w) = it.next() {
        if INLINE_OPTIONS.contains(&w) { return format!("{name} {w}"); }
        if w == "-m" { return it.next().map_or_else(|| format!("{name} -m"), |m| format!("{name} -m {m}")); }
        if VALUE_OPTIONS.contains(&w) { it.next(); continue; }
        if w.starts_with('-') { continue; }
        return format!("{name} {}", basename(w));
    }
    name.to_string()
}

/// sudo/su run setuid-root, so libproc refuses to describe them to a normal user; `ps` still can.
fn ppid_via_ps(pid: u32) -> Option<u32> {
    let out = Command::new("ps").args(["-o", "ppid=", "-p", &pid.to_string()]).output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Ancestors of `pid`, nearest first, stopping at launchd.
pub fn chain_of(pid: u32) -> Vec<Hop> {
    let mut hops = Vec::new();
    let mut cur = ppid(pid).or_else(|| ppid_via_ps(pid));
    while let Some(p) = cur {
        if p <= 1 || hops.len() >= 12 { break; }
        hops.push(Hop { pid: p, exe: proc_pid::pidpath(p as i32).unwrap_or_default(), cwd: cwd_of(p), started: started_of(p), detail: true, ..Hop::default() });
        cur = ppid(p);
    }
    let rows = ps_rows(&hops.iter().map(|h| h.pid).collect::<Vec<_>>());
    for h in &mut hops { if let Some((tty, cmd)) = rows.get(&h.pid) { h.tty = tty.clone(); h.command = cmd.clone(); } }
    hops
}

fn capture(pid: u32, tool: &str) -> SudoEvent {
    // Grab the chain first: the parents outlive sudo, sudo's own args may already be gone.
    let chain = chain_of(pid);
    let args = commands(&[pid]).remove(&pid).unwrap_or_else(|| "(exited before capture)".into());
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    SudoEvent { ts, pid, tool: tool.into(), args, chain }
}

pub fn watch(mut on_event: impl FnMut(SudoEvent)) -> ! {
    let mut known: HashSet<u32> = pids().into_iter().collect();
    let mut fresh: HashMap<u32, Instant> = HashMap::new();
    let mut reported: HashSet<u32> = HashSet::new();
    loop {
        std::thread::sleep(POLL);
        let now = pids();
        let live: HashSet<u32> = now.iter().copied().collect();
        for &p in &now { if known.insert(p) { fresh.insert(p, Instant::now()); } }
        known.retain(|p| live.contains(p));
        reported.retain(|p| live.contains(p));
        fresh.retain(|p, t| t.elapsed() < FRESH && live.contains(p));
        for &p in fresh.keys() {
            if reported.contains(&p) { continue; }
            if let Ok(path) = proc_pid::pidpath(p as i32) {
                if WATCHED.contains(&path.as_str()) {
                    reported.insert(p);
                    on_event(capture(p, &path));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps_rows_give_pid_terminal_and_command() {
        let m = parse_ps("  101 ttys003 /bin/zsh -l\n  102 ??       node server.js --port 1\n  103 ttys001 -zsh\n");
        assert_eq!(m[&101], (Some("ttys003".into()), "/bin/zsh -l".into()));
        assert_eq!(m[&102], (None, "node server.js --port 1".into()), "?? means no controlling terminal");
        assert_eq!(m[&103].1, "-zsh");
    }

    #[test]
    fn ps_rows_keep_a_command_with_angle_brackets_and_quotes_intact() {
        let m = parse_ps("7 ?? sh -c echo \"a<b&c\" > 'x'\n");
        assert_eq!(m[&7].1, "sh -c echo \"a<b&c\" > 'x'");
    }

    #[test]
    fn ps_rows_skip_junk_and_cut_long_commands() {
        let long = "x".repeat(COMMAND_MAX + 50);
        let m = parse_ps(&format!("not a row\n\n9 ttys000 {long}\n12\n"));
        assert_eq!(m.len(), 1);
        assert_eq!(m[&9].1.chars().count(), COMMAND_MAX);
    }

    #[test]
    fn labels_match_the_shared_fixture() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!("../tests/fixtures/hop-labels.json")).unwrap();
        assert!(cases.len() > 10);
        for c in cases {
            let (cmd, exe) = (c["command"].as_str().unwrap(), c["exe"].as_str().unwrap());
            assert_eq!(hop_label(cmd, exe), c["label"].as_str().unwrap(), "command {cmd:?} exe {exe:?}");
        }
    }

    #[test]
    fn an_old_hop_with_three_fields_still_loads() {
        let h: Hop = serde_json::from_str(r#"{"pid":5,"exe":"/bin/zsh","command":"-zsh"}"#).unwrap();
        assert_eq!((h.pid, h.started, h.tty, h.cwd, h.detail), (5, None, None, None, false));
    }

    #[test]
    fn a_new_hop_round_trips() {
        let h = Hop { pid: 5, exe: "/bin/zsh".into(), command: "zsh".into(), started: Some(10), tty: Some("ttys001".into()), cwd: Some("/tmp".into()), detail: true };
        assert_eq!(serde_json::from_str::<Hop>(&serde_json::to_string(&h).unwrap()).unwrap(), h);
    }

    #[test]
    fn own_working_directory_and_start_time_are_readable() {
        let me = std::process::id();
        assert_eq!(cwd_of(me).map(std::path::PathBuf::from).and_then(|p| p.canonicalize().ok()), std::env::current_dir().unwrap().canonicalize().ok());
        assert!(started_of(me).is_some());
        assert_eq!(cwd_of(u32::MAX - 1), None, "a pid that does not exist is unreadable, not an empty string");
    }

    #[test]
    fn the_chain_of_this_process_is_enriched() {
        let chain = chain_of(std::process::id());
        assert!(!chain.is_empty());
        assert!(chain.iter().all(|h| h.detail && h.pid > 1));
        assert!(chain[0].started.is_some() && !chain[0].command.is_empty());
    }
}
