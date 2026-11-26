//! Process actions. Deliberately narrow: signal only your own processes, never launchd or ourselves.
use serde::Serialize;
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sig { Term, Kill }
impl Sig {
    fn flag(self) -> &'static str { match self { Sig::Term => "-TERM", Sig::Kill => "-KILL" } }
    pub fn name(self) -> &'static str { match self { Sig::Term => "TERM", Sig::Kill => "KILL" } }
}

pub fn parse(pids: &str, sig: &str) -> Result<(Vec<u32>, Sig), String> {
    let sig = match sig { "TERM" => Sig::Term, "KILL" => Sig::Kill, _ => return Err("sig must be TERM or KILL".into()) };
    let v = pids.split(',').filter(|s| !s.is_empty())
        .map(|s| s.parse::<u32>().map_err(|_| format!("bad pid '{s}'"))).collect::<Result<Vec<_>, _>>()?;
    if v.is_empty() || v.len() > 200 { return Err("expected 1 to 200 pids".into()); }
    Ok((v, sig))
}

#[derive(Serialize, Debug)]
pub struct Refused { pub pid: u32, pub reason: String }
#[derive(Serialize, Debug, Default)]
pub struct Outcome { pub sent: Vec<u32>, pub refused: Vec<Refused> }

fn out(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

fn check(pid: u32, my_uid: &str) -> Result<(), &'static str> {
    if pid <= 1 { return Err("refusing to signal launchd"); }
    if pid == std::process::id() { return Err("refusing to signal sys-monitor itself"); }
    let uid = out("ps", &["-o", "uid=", "-p", &pid.to_string()]);
    if uid.is_empty() { Err("no such process") } else if uid != my_uid { Err("not your process") } else { Ok(()) }
}

pub fn execute(pids: &[u32], sig: Sig) -> Outcome {
    let me = out("id", &["-u"]);
    let mut o = Outcome::default();
    for &pid in pids {
        match check(pid, &me) {
            Err(reason) => o.refused.push(Refused { pid, reason: reason.into() }),
            Ok(()) => match Command::new("kill").args([sig.flag(), &pid.to_string()]).status() {
                Ok(s) if s.success() => o.sent.push(pid),
                _ => o.refused.push(Refused { pid, reason: "kill failed".into() }),
            },
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_requests() {
        assert_eq!(parse("12,34", "TERM").unwrap(), (vec![12, 34], Sig::Term));
        assert!(parse("12", "HUP").is_err());
        assert!(parse("", "TERM").is_err());
        assert!(parse("1;rm", "KILL").is_err());
    }
    #[test]
    fn refuses_launchd_and_self() {
        let o = execute(&[1, std::process::id()], Sig::Term);
        assert!(o.sent.is_empty() && o.refused.len() == 2);
    }
}
