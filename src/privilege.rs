use serde::Serialize;
use std::process::Command;

#[derive(Serialize, Debug, PartialEq)]
pub struct PrivEvent {
    pub time: String, pub right: String, pub client: String, pub creator: String, pub pid: u32,
    pub ok: bool, pub error: Option<String>,
    /// requester's parent, only resolvable while the process is still alive
    pub parent: Option<String>,
}

fn between<'a>(s: &'a str, a: &str, b: &str) -> Option<&'a str> {
    let i = s.find(a)? + a.len();
    let j = s[i..].find(b)? + i;
    Some(&s[i..j])
}

/// Parses authd messages such as
/// "Failed to authorize right 'R' by client 'C' [123] for authorization created by 'X' [123] (3,0) (-60006)".
pub fn parse_message(time: &str, msg: &str) -> Option<PrivEvent> {
    let ok = msg.contains("Succeeded authorizing right");
    if !ok && !msg.contains("Failed to authorize right") { return None; }
    let right = between(msg, "right '", "'")?.to_string();
    let client = between(msg, "by client '", "'")?.to_string();
    let after = &msg[msg.find("by client '")?..];
    let pid = between(after, "' [", "]")?.parse().ok()?;
    let creator = between(msg, "created by '", "'")?.to_string();
    let error = (!ok).then(|| between(msg, ") (", ")").unwrap_or("failed").to_string());
    Some(PrivEvent { time: time.into(), right, client, creator, pid, ok, error, parent: None })
}

fn parent_of(pid: u32) -> Option<String> {
    let out = Command::new("ps").args(["-o", "ppid=,command=", "-p", &pid.to_string()]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Authorization requests (sudo, admin prompts, system settings unlocks) from the last `minutes`.
pub fn recent(minutes: u32) -> Vec<PrivEvent> {
    let out = Command::new("/usr/bin/log")
        .args(["show", "--last", &format!("{minutes}m"), "--info", "--style", "ndjson", "--predicate",
               "process == \"authd\" AND (eventMessage CONTAINS \"authorize right\" OR eventMessage CONTAINS \"authorizing right\")"])
        .output();
    let Ok(out) = out else { return vec![] };
    let mut evs: Vec<PrivEvent> = String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| {
        let v: serde_json::Value = serde_json::from_str(l).ok()?;
        let mut e = parse_message(v["timestamp"].as_str()?, v["eventMessage"].as_str()?)?;
        e.parent = parent_of(e.pid);
        Some(e)
    }).collect();
    evs.reverse(); // newest first
    evs
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_failed_sudo() {
        let m = "Failed to authorize right 'com.apple.security.sudo' by client '/usr/bin/sudo' [66003] for authorization created by '/usr/bin/sudo' [66003] (3,0) (-60006) (engine 2418)";
        let e = parse_message("t", m).unwrap();
        assert_eq!((e.right.as_str(), e.pid, e.ok), ("com.apple.security.sudo", 66003, false));
        assert_eq!(e.client, "/usr/bin/sudo");
        assert_eq!(e.error.as_deref(), Some("-60006"));
    }
    #[test]
    fn ignores_noise() { assert!(parse_message("t", "copy_rights: authorization failed").is_none()); }
}
