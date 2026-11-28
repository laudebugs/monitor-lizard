//! Starts, supervises and health-checks a local `llama-server` (llama.cpp's own binary, not built
//! here) so `/api/classify` always has a quick, true answer to "can I call the model right now" --
//! never a hang, and never more than the Scope's loopback-only, cleared-environment contract. A
//! background thread (`start`) owns the one child process; every other caller only ever reads
//! `LlmServer::base_url_if_ready` or `LlmServer::reason`, never touches the child directly.
use crate::{model_store, sampler, settings};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Context size is fixed, not read from the model's own (much larger) advertised window: a
/// conservative default keeps memory use predictable on an 8 GB Mac, which the todo's own Risks
/// section asks to measure -- not yet measured against a real binary in this session (see the
/// Implementation Record), so this stays deliberately modest rather than assumed safe at a larger size.
const CTX_SIZE: &str = "4096";
const HEALTH_TIMEOUT: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const STARTUP_RETRY: Duration = Duration::from_secs(2);
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// How long to wait before re-checking a reason that rarely changes between one tick and the next.
/// `model_store::status` re-hashes the whole (multi-gigabyte) model file on every call, so retrying
/// that check every couple of seconds the way a crash-backoff does would mean re-hashing it that
/// often for as long as the model stays uninstalled -- real disk and CPU cost for no new information.
const SLOW_RETRY: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotReadyReason {
    /// The kill switch (`monitor-lizard agent off`, the default).
    Disabled,
    ModelNotInstalled,
    MemoryPressure,
    BinaryNotFound,
    /// Spawned; health check has not yet passed (or has not been tried this tick).
    Starting,
    /// The child exited since it was last seen running; waiting out the backoff before retrying.
    Crashed,
}

impl NotReadyReason {
    /// A short, stable string for the HTTP response and CLI output -- never the `Debug` form, which
    /// is free to change with the enum's own variant names.
    pub fn as_str(self) -> &'static str {
        match self {
            NotReadyReason::Disabled => "disabled",
            NotReadyReason::ModelNotInstalled => "model-not-installed",
            NotReadyReason::MemoryPressure => "memory-pressure",
            NotReadyReason::BinaryNotFound => "binary-not-found",
            NotReadyReason::Starting => "starting",
            NotReadyReason::Crashed => "crashed",
        }
    }
}

/// What `tick()` decides before touching a process at all: whether to start was already true two
/// polls ago, so this only reflects what changed, free of `Instant`/`Child` so it is directly
/// testable with plain values.
fn preflight(enabled: bool, pressure_level: Option<u8>, model_path: Option<&Path>, binary: Option<&Path>) -> Result<(PathBuf, PathBuf), NotReadyReason> {
    if !enabled { return Err(NotReadyReason::Disabled); }
    if pressure_level == Some(4) { return Err(NotReadyReason::MemoryPressure); }
    let model_path = model_path.ok_or(NotReadyReason::ModelNotInstalled)?;
    let binary = binary.ok_or(NotReadyReason::BinaryNotFound)?;
    Ok((model_path.to_path_buf(), binary.to_path_buf()))
}

/// The installed, hash-verified model file to load, preferring `model_store::DEFAULT_MODEL` but
/// accepting any pinned entry that verifies -- a Mac that installed only the smaller fallback must
/// still be able to run the agent.
fn installed_model_path() -> Option<PathBuf> {
    model_store::MANIFEST.iter()
        .find(|m| matches!(model_store::status(m), model_store::StatusReport::Verified(_)))
        .map(model_store::install_path)
}

/// Homebrew's own install prefixes (Apple Silicon, then Intel) checked first since they're exact
/// paths and skip a `PATH` split; then every directory on `PATH`, Homebrew's own usual place for a
/// plain `brew install llama.cpp` to put it. No bundling, no download of the binary itself: this
/// repo only verifies and fetches the model file (`model_store.rs`), never an executable.
fn locate_llama_server() -> Option<PathBuf> {
    for candidate in ["/opt/homebrew/bin/llama-server", "/usr/local/bin/llama-server"] {
        let p = PathBuf::from(candidate);
        if p.is_file() { return Some(p); }
    }
    let path_var = std::env::var("PATH").ok()?;
    std::env::split_paths(&path_var).map(|dir| dir.join("llama-server")).find(|p| p.is_file())
}

/// A free loopback port, picked by letting the OS choose and immediately releasing it. The gap
/// between release and `llama-server` binding it is a real but small race on a single-user,
/// loopback-only tool: a loss just means this attempt's health check fails like any other failed
/// start, and the supervisor retries with backoff the same as it would for a crash.
fn free_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0").ok()?.local_addr().ok().map(|a| a.port())
}

/// A private scratch directory created for this run, to use as the child's `HOME` instead of the
/// real one. `llama-server` is always pointed at the model by its absolute path (see `tick()`), so
/// it never needs the real `HOME` to find anything; handing over the real one anyway would let a
/// third-party binary read SSH keys, cloud credentials and dotfiles it has no reason to see.
fn scratch_home() -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("monitor-lizard-agent-home-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Spawns `program` with a cleared environment plus a narrow allowlist (never the inherited
/// environment, which may carry credentials this subprocess has no business seeing): `PATH` so the
/// binary's own dynamic linking and any tools it shells out to still resolve, a scratch `HOME` (see
/// `scratch_home`), and `TMPDIR`/`LANG` passed through unchanged since neither carries anything
/// sensitive. No inherited stdio: `llama-server`'s own logging is not captured in this first cut
/// (see the Implementation Record's Open note).
fn spawn_with_clean_env(program: &Path, args: &[&str]) -> std::io::Result<Child> {
    let home = scratch_home()?;
    let mut cmd = Command::new(program);
    cmd.env_clear();
    if let Ok(v) = std::env::var("PATH") { cmd.env("PATH", v); }
    cmd.env("HOME", home);
    for key in ["TMPDIR", "LANG"] {
        if let Ok(v) = std::env::var(key) { cmd.env(key, v); }
    }
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn()
}

/// `true` once `child` has exited, however that happened; never blocks.
fn has_exited(child: &mut Child) -> bool {
    matches!(child.try_wait(), Ok(Some(_)))
}

/// Kills and reaps `child`, ignoring both results: by the time this is called the goal is only
/// "make sure it's gone," not to diagnose how the kill or the wait went.
fn kill_and_wait(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// `GET <base_url>/health` within `HEALTH_TIMEOUT`: llama.cpp's own server convention for "loaded
/// and ready to serve" (unverified against a real binary in this session; see the Implementation
/// Record). Any non-2xx status, or no reply at all, means not ready yet -- never an error upward.
fn health_check(base_url: &str) -> bool {
    let config = ureq::Agent::config_builder().timeout_global(Some(HEALTH_TIMEOUT)).http_status_as_error(false).build();
    let agent: ureq::Agent = config.into();
    agent.get(format!("{base_url}/health")).call().is_ok_and(|r| r.status().is_success())
}

struct Inner {
    child: Option<Child>,
    port: u16,
    ready: bool,
    reason: NotReadyReason,
    next_attempt: Instant,
    backoff: Duration,
}

/// The one handle a running server hands out; `/api/classify` and `monitor-lizard agent` both only
/// ever read through this, never reach into the supervisor thread's own state.
pub struct LlmServer {
    inner: Mutex<Inner>,
}

impl LlmServer {
    /// `Some(base_url)` only once the child is spawned and has passed a health check; `None` with
    /// no further detail -- call `reason()` for why.
    pub fn base_url_if_ready(&self) -> Option<String> {
        let g = self.inner.lock().unwrap();
        g.ready.then(|| format!("http://127.0.0.1:{}", g.port))
    }

    pub fn reason(&self) -> NotReadyReason {
        self.inner.lock().unwrap().reason
    }

    /// Stops the child immediately if one is running, and marks the server `Disabled` so the next
    /// tick does not just restart it. Used by `agent off` while a server process in this same run
    /// is live; a fresh process picks up the persisted setting as `Disabled` from its very first tick.
    fn apply_disabled(&self) {
        let mut g = self.inner.lock().unwrap();
        if let Some(child) = g.child.take() { kill_and_wait(child); }
        g.ready = false;
        g.reason = NotReadyReason::Disabled;
        g.next_attempt = Instant::now() + STARTUP_RETRY;
    }

    fn tick(&self) {
        let enabled = settings::load().agent_mode.runs_model();
        if !enabled {
            // Idempotent: killing an already-absent child and re-setting an already-Disabled
            // reason both cost nothing, so there is no need to check first whether anything changed.
            self.apply_disabled();
            return;
        }
        let mut g = self.inner.lock().unwrap();
        if let Some(child) = g.child.as_mut() {
            if has_exited(child) {
                g.child = None;
                g.ready = false;
                g.reason = NotReadyReason::Crashed;
                g.backoff = (g.backoff * 2).min(MAX_BACKOFF);
                g.next_attempt = Instant::now() + g.backoff;
                return;
            }
            if !g.ready {
                let port = g.port;
                drop(g); // health_check makes a network call; never hold the lock across it
                if health_check(&format!("http://127.0.0.1:{port}")) {
                    let mut g = self.inner.lock().unwrap();
                    g.ready = true;
                    g.backoff = MIN_BACKOFF;
                }
            }
            return;
        }
        if Instant::now() < g.next_attempt { return; }
        let pressure = match sampler::pressure_level() { sampler::Reading::Value(v) => Some(v), _ => None };
        let model_path = installed_model_path();
        let binary = locate_llama_server();
        match preflight(enabled, pressure, model_path.as_deref(), binary.as_deref()) {
            Err(reason) => {
                g.reason = reason;
                let delay = match reason {
                    NotReadyReason::ModelNotInstalled | NotReadyReason::BinaryNotFound => SLOW_RETRY,
                    _ => STARTUP_RETRY,
                };
                g.next_attempt = Instant::now() + delay;
            }
            Ok((model_path, binary)) => {
                let Some(port) = free_port() else {
                    g.reason = NotReadyReason::Crashed;
                    g.next_attempt = Instant::now() + g.backoff;
                    return;
                };
                let model_arg = model_path.to_string_lossy().into_owned();
                let args = ["--host", "127.0.0.1", "--port", &port.to_string(), "--model", &model_arg, "--ctx-size", CTX_SIZE];
                match spawn_with_clean_env(&binary, &args) {
                    Ok(child) => {
                        g.child = Some(child);
                        g.port = port;
                        g.ready = false;
                        g.reason = NotReadyReason::Starting;
                    }
                    Err(_) => {
                        g.reason = NotReadyReason::Crashed;
                        g.backoff = (g.backoff * 2).min(MAX_BACKOFF);
                        g.next_attempt = Instant::now() + g.backoff;
                    }
                }
            }
        }
    }
}

/// Starts the supervisor's background thread and returns the handle every request thread reads
/// from. Exactly one of these should exist per running `monitor-lizard serve`/shell, the same as
/// `sampler::start()`.
pub fn start() -> std::sync::Arc<LlmServer> {
    let srv = std::sync::Arc::new(LlmServer {
        inner: Mutex::new(Inner { child: None, port: 0, ready: false, reason: NotReadyReason::Disabled, next_attempt: Instant::now(), backoff: MIN_BACKOFF }),
    });
    let handle = srv.clone();
    std::thread::spawn(move || loop {
        handle.tick();
        std::thread::sleep(POLL_INTERVAL);
    });
    srv
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tiny_http::{Response, Server};

    #[test]
    fn preflight_reports_the_first_reason_it_finds_in_priority_order() {
        let path = Path::new("/tmp/model.gguf");
        let bin = Path::new("/tmp/llama-server");
        assert_eq!(preflight(false, Some(1), Some(path), Some(bin)), Err(NotReadyReason::Disabled), "disabled wins over everything else");
        assert_eq!(preflight(true, Some(4), Some(path), Some(bin)), Err(NotReadyReason::MemoryPressure));
        assert_eq!(preflight(true, Some(1), None, Some(bin)), Err(NotReadyReason::ModelNotInstalled));
        assert_eq!(preflight(true, Some(1), Some(path), None), Err(NotReadyReason::BinaryNotFound));
        assert_eq!(preflight(true, None, Some(path), Some(bin)), Ok((path.to_path_buf(), bin.to_path_buf())), "an unreadable pressure level must not block starting; only a confirmed level 4 does");
        assert_eq!(preflight(true, Some(2), Some(path), Some(bin)), Ok((path.to_path_buf(), bin.to_path_buf())), "warn (2) is not critical (4): the agent still runs");
    }

    #[test]
    fn a_still_running_child_is_not_reported_exited() {
        let mut child = Command::new("/bin/sh").args(["-c", "sleep 1"]).spawn().expect("spawn sleep");
        assert!(!has_exited(&mut child), "a child mid-sleep has not exited yet");
        kill_and_wait(child);
    }

    #[test]
    fn an_exited_child_is_detected_without_blocking() {
        let mut child = Command::new("/bin/sh").args(["-c", "exit 0"]).spawn().expect("spawn true");
        let start = Instant::now();
        // try_wait is non-blocking, but the child needs a moment to actually finish after spawn.
        let exited = (0..200).any(|_| { if has_exited(&mut child) { true } else { thread::sleep(Duration::from_millis(5)); false } });
        assert!(exited, "a trivially-exiting child should be detected well within a second");
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn kill_and_wait_ends_a_long_running_child_promptly() {
        let child = Command::new("/bin/sh").args(["-c", "sleep 100"]).spawn().expect("spawn sleep 100");
        let start = Instant::now();
        kill_and_wait(child);
        assert!(start.elapsed() < Duration::from_secs(5), "kill_and_wait must not wait out the child's own sleep");
    }

    #[test]
    fn the_cleared_environment_drops_an_arbitrary_variable_but_keeps_path_working() {
        // SAFETY (set_var): this test does not run concurrently with any other code reading env
        // vars in this process in a way that matters -- it only asserts on this one child's view.
        unsafe { std::env::set_var("MONITOR_LIZARD_TEST_SECRET", "should-not-be-inherited"); }
        let mut child = spawn_with_clean_env(Path::new("/bin/sh"), &["-c", "test -z \"$MONITOR_LIZARD_TEST_SECRET\" && test -n \"$PATH\""]).expect("spawn sh");
        let status = child.wait().expect("wait");
        unsafe { std::env::remove_var("MONITOR_LIZARD_TEST_SECRET"); }
        assert!(status.success(), "the child must see PATH (allowlisted) but not the arbitrary var (cleared)");
    }

    #[test]
    fn the_real_home_is_never_handed_to_the_child() {
        let real_home = std::env::var("HOME").unwrap_or_default();
        let script = format!("[ \"$HOME\" != \"{real_home}\" ] && [ -n \"$HOME\" ]");
        let mut child = spawn_with_clean_env(Path::new("/bin/sh"), &["-c", &script]).expect("spawn sh");
        let status = child.wait().expect("wait");
        assert!(status.success(), "the child's HOME must be some scratch directory, never the real one, and never empty");
    }

    /// A tiny fixture HTTP server standing in for `llama-server`'s `/health` endpoint, the same
    /// pattern `src/llm.rs`'s own tests use for its chat-completions endpoint.
    fn fixture_server(status: u16) -> String {
        let server = Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        thread::spawn(move || {
            for req in server.incoming_requests() {
                let _ = req.respond(Response::from_string("{}").with_status_code(status));
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    #[test]
    fn health_check_is_true_only_on_a_2xx_reply() {
        assert!(health_check(&fixture_server(200)));
        assert!(!health_check(&fixture_server(503)));
    }

    #[test]
    fn health_check_is_false_when_nothing_is_listening() {
        assert!(!health_check("http://127.0.0.1:1"), "port 1 refuses on loopback, standing in for a not-yet-started server");
    }
}
