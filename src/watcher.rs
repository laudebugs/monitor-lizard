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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hop { pub pid: u32, pub exe: String, pub command: String }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SudoEvent { pub ts: u64, pub pid: u32, pub tool: String, pub args: String, pub chain: Vec<Hop> }

fn ppid(pid: u32) -> Option<u32> { proc_pid::pidinfo::<BSDInfo>(pid as i32, 0).ok().map(|b| b.pbi_ppid) }
fn pids() -> Vec<u32> { pids_by_type(ProcFilter::All).unwrap_or_default() }

fn commands(pids: &[u32]) -> HashMap<u32, String> {
    if pids.is_empty() { return HashMap::new(); }
    let list = pids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let out = Command::new("ps").args(["-ww", "-o", "pid=,command=", "-p", &list]).output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    text.lines().filter_map(|l| {
        let (p, c) = l.trim().split_once(char::is_whitespace)?;
        Some((p.parse().ok()?, c.trim().chars().take(300).collect()))
    }).collect()
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
        hops.push(Hop { pid: p, exe: proc_pid::pidpath(p as i32).unwrap_or_default(), command: String::new() });
        cur = ppid(p);
    }
    let cmds = commands(&hops.iter().map(|h| h.pid).collect::<Vec<_>>());
    for h in &mut hops { h.command = cmds.get(&h.pid).cloned().unwrap_or_default(); }
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
