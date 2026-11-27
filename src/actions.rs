//! Process actions. Deliberately narrow: signal only your own processes, never launchd or ourselves.
use crate::storage::trash::MoveError;
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

/// Shared with `classify::allowed_actions`, so there is exactly one place that decides whether a
/// pid can be signalled: a live `ps` lookup, not a cached snapshot field, so it can never go stale.
pub(crate) fn check(pid: u32, my_uid: &str) -> Result<(), &'static str> {
    if pid <= 1 { return Err("refusing to signal launchd"); }
    if pid == std::process::id() { return Err("refusing to signal monitor-lizard itself"); }
    let uid = out("ps", &["-o", "uid=", "-p", &pid.to_string()]);
    if uid.is_empty() { Err("no such process") } else if uid != my_uid { Err("not your process") } else { Ok(()) }
}

pub(crate) fn my_uid() -> String { out("id", &["-u"]) }
pub(crate) fn my_username() -> String { out("id", &["-un"]) }

pub fn execute(pids: &[u32], sig: Sig) -> Outcome {
    let me = my_uid();
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

/// Moves one path to the user's Trash through `NSFileManager`. The Finder route keeps "Put Back" but
/// asks for an Automation permission the first time, so it is not used. The caller has already
/// checked the path; this only does the move and returns the OS's reason when it fails.
pub fn trash(path: &std::path::Path) -> Result<(), MoveError> {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    let mut ctx = trash::TrashContext::default();
    ctx.set_delete_method(DeleteMethod::NsFileManager);
    ctx.delete(path).map_err(move_error)
}

fn move_error(e: trash::Error) -> MoveError {
    match &e {
        trash::Error::Os { code, .. } if *code == libc::ENOENT => MoveError::NotFound,
        trash::Error::Os { code, description } if *code == libc::EPERM || *code == libc::EACCES => MoveError::NotPermitted(description.clone()),
        _ => MoveError::Other(e.to_string()),
    }
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
    #[test]
    fn mover_errors_are_typed_by_their_os_code() {
        let os = |code| trash::Error::Os { code, description: "d".into() };
        assert_eq!(move_error(os(libc::ENOENT)), MoveError::NotFound);
        assert_eq!(move_error(os(libc::EPERM)), MoveError::NotPermitted("d".into()));
        assert_eq!(move_error(os(libc::EACCES)), MoveError::NotPermitted("d".into()));
        assert!(matches!(move_error(os(libc::EBUSY)), MoveError::Other(_)));
    }
    /// Touches the real Trash, so it only runs on request: `cargo test --lib real_trash -- --ignored`.
    #[test]
    #[ignore = "puts a file in the real Trash"]
    fn real_trash_takes_a_file_and_refuses_a_missing_one() {
        let p = std::env::temp_dir().join(format!("ml-real-trash-{}.txt", std::process::id()));
        std::fs::write(&p, "monitor-lizard trash test").unwrap();
        trash(&p).unwrap();
        assert!(!p.exists());
        assert_eq!(trash(&p).map_err(|e| matches!(e, MoveError::NotFound | MoveError::Other(_))), Err(true));
    }
}
