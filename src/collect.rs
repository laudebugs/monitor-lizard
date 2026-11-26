use crate::model::*;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

fn run(cmd: &str, args: &[&str]) -> String {
    match Command::new(cmd).args(args).output() {
        Ok(o) => format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
        Err(_) => String::new(),
    }
}

// ---------- processes ----------
pub fn processes() -> Vec<Proc> {
    let mut procs = parse_ps(&run("ps", &["-axww", "-o", "pid=,ppid=,user=,pcpu=,rss=,command="]));
    // `ps` shows argv[0], which programs can rewrite; the kernel's proc_pidpath is the real image path.
    for p in &mut procs {
        if let Ok(path) = libproc::proc_pid::pidpath(p.pid as i32) { p.exe = path; }
    }
    procs
}

fn next_token(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() { return None; }
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    Some((&s[..end], s[end..].trim_start()))
}

pub fn parse_ps(out: &str) -> Vec<Proc> {
    out.lines().filter_map(|line| {
        let (pid, r) = next_token(line)?;
        let (ppid, r) = next_token(r)?;
        let (user, r) = next_token(r)?;
        let (cpu, r) = next_token(r)?;
        let (rss, command) = next_token(r)?;
        Some(Proc {
            pid: pid.parse().ok()?, ppid: ppid.parse().ok()?, user: user.into(),
            cpu: cpu.parse().ok()?, rss_kb: rss.parse().ok()?,
            exe: exe_of(command), command: command.into(),
        })
    }).collect()
}

/// Executable path of a `ps` command line; handles spaces inside .app bundles.
pub fn exe_of(command: &str) -> String {
    const MARK: &str = ".app/Contents/MacOS/";
    if let Some(i) = command.find(MARK) {
        let rest = &command[i + MARK.len()..];
        let end = rest.find(" -").unwrap_or(rest.len());
        return command[..i + MARK.len() + end].to_string();
    }
    command.split(" -").next().unwrap_or(command).split_whitespace().next().unwrap_or(command).to_string()
}

// ---------- sockets ----------
pub fn scope_of(addr: &str) -> Scope {
    let a = addr.trim_matches(|c| c == '[' || c == ']');
    if a == "127.0.0.1" || a == "::1" || a == "localhost" || a == "*" { return Scope::Loopback; }
    let lan = a.starts_with("10.") || a.starts_with("192.168.") || a.starts_with("169.254.")
        || a.split('.').next() == Some("172") && a.split('.').nth(1).and_then(|n| n.parse::<u8>().ok()).is_some_and(|n| (16..=31).contains(&n))
        || ["fe80", "fc", "fd"].iter().any(|p| a.to_lowercase().starts_with(p));
    if lan { Scope::Lan } else { Scope::Public }
}

fn host_port(s: &str) -> (String, u16) {
    match s.rsplit_once(':') { Some((h, p)) => (h.into(), p.parse().unwrap_or(0)), None => (s.into(), 0) }
}

pub fn connections() -> Vec<Conn> { parse_lsof(&run("lsof", &["-nP", "-i", "-F", "pcPnTt"])) }

/// Parses `lsof -F pcPnTt` field output.
pub fn parse_lsof(out: &str) -> Vec<Conn> {
    let mut conns = Vec::new();
    let (mut pid, mut name, mut proto, mut state, mut pending) = (0u32, String::new(), String::new(), String::new(), String::new());
    let mut flush = |pid: u32, name: &str, proto: &str, state: &str, pending: &mut String| {
        if pending.is_empty() { return; }
        let (l, r) = match pending.split_once("->") { Some((l, r)) => (l.to_string(), Some(r.to_string())), None => (pending.clone(), None) };
        let (local_addr, local_port) = host_port(&l);
        let remote = r.as_deref().map(host_port);
        let listen = state == "LISTEN" || (proto == "UDP" && remote.is_none());
        let scope = scope_of(remote.as_ref().map(|r| r.0.as_str()).unwrap_or(&local_addr));
        let exposed = listen && ["*", "0.0.0.0", "[::]"].contains(&local_addr.as_str());
        let direction = if listen { Direction::Listen } else if scope == Scope::Loopback { Direction::Local } else { Direction::Outbound };
        conns.push(Conn {
            pid, name: name.into(), proto: proto.into(),
            state: if !state.is_empty() { state.into() } else if remote.is_some() { "ESTABLISHED".into() } else { "BOUND".into() },
            local_addr, local_port, remote_addr: remote.as_ref().map(|r| r.0.clone()), remote_port: remote.map(|r| r.1),
            direction, scope, exposed,
        });
        pending.clear();
    };
    for line in out.lines() {
        let Some(tag) = line.chars().next() else { continue };
        let val = &line[1..];
        match tag {
            'p' => { flush(pid, &name, &proto, &state, &mut pending); pid = val.parse().unwrap_or(0); state.clear(); }
            'c' => name = val.into(),
            'f' => { flush(pid, &name, &proto, &state, &mut pending); state.clear(); }
            'P' => proto = val.into(),
            'n' => pending = val.into(),
            'T' => if let Some(s) = val.strip_prefix("ST=") { state = s.into() },
            _ => {}
        }
    }
    flush(pid, &name, &proto, &state, &mut pending);
    conns
}

// ---------- code signing ----------
static CACHE: OnceLock<Mutex<HashMap<String, Signing>>> = OnceLock::new();

pub fn signing_of(path: &str) -> Signing {
    let cache = CACHE.get_or_init(Default::default);
    if let Some(s) = cache.lock().unwrap().get(path) { return s.clone(); }
    let out = run("codesign", &["-dv", "--verbose=2", path]);
    let field = |k: &str| out.lines().find_map(|l| l.strip_prefix(k)).map(str::to_string);
    let team = field("TeamIdentifier=").filter(|t| t != "not set");
    let authority = field("Authority=");
    let signed = authority.is_some() || out.contains("Signature size");
    let apple = !out.contains("not signed") && (out.contains("Software Signing") || out.contains("Apple Root CA")) && team.is_none();
    let s = Signing { signed: signed && !out.contains("not signed"), team, authority, apple };
    cache.lock().unwrap().insert(path.into(), s.clone());
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exe_with_spaces() {
        assert_eq!(exe_of("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome --type=gpu"), "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
        assert_eq!(exe_of("/usr/sbin/mDNSResponder"), "/usr/sbin/mDNSResponder");
    }
    #[test]
    fn lsof_outbound_and_listen() {
        let out = "p42\ncSpotify\nf10\ntIPv4\nPTCP\nn*:57621\nTST=LISTEN\nf11\ntIPv4\nPTCP\nn10.0.0.2:5000->8.8.8.8:443\nTST=ESTABLISHED\n";
        let c = parse_lsof(out);
        assert_eq!(c.len(), 2);
        assert!(c[0].exposed && c[0].direction == Direction::Listen);
        assert_eq!(c[1].direction, Direction::Outbound);
        assert_eq!(c[1].scope, Scope::Public);
    }
    #[test]
    fn scopes() {
        assert_eq!(scope_of("192.168.1.5"), Scope::Lan);
        assert_eq!(scope_of("172.20.0.1"), Scope::Lan);
        assert_eq!(scope_of("172.32.0.1"), Scope::Public);
        assert_eq!(scope_of("[fe80::1]"), Scope::Lan);
    }
}
