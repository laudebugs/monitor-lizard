//! Downloads and verifies the local classifier model. The repo, commit, file name, size, SHA-256
//! and licence are pinned in `MANIFEST` below, in this binary, not fetched from the Hub at
//! runtime: a compromised or silently swapped Hub file is refused, never trusted on the Hub's say-so.
//! Running the model is a later module's job; this one only gets a verified file onto disk.
use crate::paths::{create_private_dir, create_private_file, create_private_file_truncating};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct ModelEntry {
    /// The `--model` value naming this entry.
    pub id: &'static str,
    pub repo: &'static str,
    /// The commit this file is fetched at, so a later push to the repo's default branch cannot
    /// change what gets downloaded. Obtained from the Hugging Face API's own `sha` field for the repo.
    pub commit: &'static str,
    pub file_name: &'static str,
    pub size_bytes: u64,
    /// Lower-case hex. Obtained by reading the file's own git-lfs pointer (`.../raw/<commit>/<file>`,
    /// which for an LFS-tracked file is the small pointer text, not the multi-gigabyte blob), whose
    /// `oid sha256:<hex>` line is the git-lfs spec's own content hash
    /// (<https://git-lfs.github.com/spec/v1>) -- not a Hub-specific field, so this does not depend
    /// on an undocumented API shape.
    pub sha256: &'static str,
    pub licence: &'static str,
}

pub const MANIFEST: &[ModelEntry] = &[
    ModelEntry {
        id: "qwen3-4b",
        repo: "Qwen/Qwen3-4B-GGUF",
        commit: "bc640142c66e1fdd12af0bd68f40445458f3869b",
        file_name: "Qwen3-4B-Q4_K_M.gguf",
        size_bytes: 2_497_280_256,
        sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
        licence: "Apache-2.0",
    },
    ModelEntry {
        id: "qwen3-1.7b",
        repo: "Qwen/Qwen3-1.7B-GGUF",
        commit: "90862c4b9d2787eaed51d12237eafdfe7c5f6077",
        file_name: "Qwen3-1.7B-Q8_0.gguf",
        size_bytes: 1_834_426_016,
        sha256: "061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a",
        licence: "Apache-2.0",
    },
];

pub const DEFAULT_MODEL: &str = "qwen3-4b";
/// Below this, `install` refuses before touching the network: the largest pinned file is 2.5 GB,
/// so this leaves headroom for its temporary file plus some margin, not a tight fit against it.
pub const MIN_FREE_BYTES: u64 = 4_000_000_000;
const TIMEOUT: Duration = Duration::from_secs(30 * 60);
const CHUNK_BYTES: usize = 256 * 1024;

pub fn find(id: &str) -> Option<&'static ModelEntry> {
    MANIFEST.iter().find(|m| m.id == id)
}

fn models_dir_in(data_dir: &Path) -> PathBuf {
    data_dir.join("models")
}

fn install_path_in(data_dir: &Path, entry: &ModelEntry) -> PathBuf {
    models_dir_in(data_dir).join(entry.file_name)
}

fn temp_path_in(data_dir: &Path, entry: &ModelEntry) -> PathBuf {
    models_dir_in(data_dir).join(format!("{}.partial", entry.file_name))
}

/// Where `install` puts a verified file, under the real data directory.
pub fn install_path(entry: &ModelEntry) -> PathBuf {
    install_path_in(&crate::paths::data_dir(), entry)
}

fn hub_url(entry: &ModelEntry) -> String {
    format!("https://huggingface.co/{}/resolve/{}/{}", entry.repo, entry.commit, entry.file_name)
}

#[derive(Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    /// `install_path` already held a file matching the manifest hash; nothing was downloaded.
    AlreadyInstalled,
    Installed,
}

#[derive(Debug)]
pub enum InstallError {
    DiskSpace { free_bytes: u64, min_bytes: u64 },
    /// Connection failed, DNS/host lookup failed, or another transport-level failure.
    Network(String),
    /// Reached the server, but it replied with an error status.
    Status(u16),
    /// The downloaded file's SHA-256 did not match the manifest. The temp file is deleted before
    /// this is returned, so a mismatch never leaves a partial file lying around to retry against.
    HashMismatch { expected: String, got: String },
    Io(String),
}

/// Pure so the 4 GB rule is testable without a real filesystem: `install` is the only caller that
/// supplies a real `free_bytes` reading.
fn check_disk_space(free_bytes: u64) -> Result<(), InstallError> {
    if free_bytes < MIN_FREE_BYTES { return Err(InstallError::DiskSpace { free_bytes, min_bytes: MIN_FREE_BYTES }); }
    Ok(())
}

/// Free bytes on the filesystem that holds `dir`, which must exist. `None` on any `statvfs` failure;
/// callers treat that the same as "not enough space" rather than skipping the check.
fn free_bytes_at(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `c` is a valid NUL-terminated path; `stat` is only read after `statvfs` returns 0,
    // which is the contract under which it has fully initialized the struct.
    let rc = unsafe { libc::statvfs(c.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 { return None; }
    // SAFETY: just checked rc == 0 above.
    let s = unsafe { stat.assume_init() };
    Some(s.f_bavail as u64 * s.f_frsize as u64)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; CHUNK_BYTES];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// Streams `url` into `dest`, resuming from `dest`'s current length with a `Range` request when it
/// is non-empty. If the server ignores the `Range` header and replies 200 instead of 206, the
/// partial bytes already on disk cannot be trusted as a prefix of this response, so `dest` is
/// truncated and the whole file is written again from this response -- never appended blindly.
fn stream_to(url: &str, dest: &Path) -> Result<(), InstallError> {
    let existing = fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    let config = ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).http_status_as_error(false).build();
    let agent: ureq::Agent = config.into();
    let mut req = agent.get(url);
    if existing > 0 { req = req.header("Range", format!("bytes={existing}-")); }
    let mut resp = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::Timeout(_)) => return Err(InstallError::Network("timed out".into())),
        Err(ureq::Error::ConnectionFailed | ureq::Error::HostNotFound | ureq::Error::Io(_)) => return Err(InstallError::Network("connection failed".into())),
        Err(e) => return Err(InstallError::Network(e.to_string())),
    };
    let status = resp.status().as_u16();
    let resumed = status == 206;
    if status != 200 && !resumed { return Err(InstallError::Status(status)); }
    // `create_private_file`/`_truncating` (shared with the rest of `~/.monitor-lizard/`) keep this
    // file mode 0600 from the moment it is created, same as every other file this app writes.
    let mut file = if resumed {
        let mut f = create_private_file(dest).map_err(|e| InstallError::Io(e.to_string()))?;
        f.seek(SeekFrom::End(0)).map_err(|e| InstallError::Io(e.to_string()))?;
        f
    } else {
        create_private_file_truncating(dest).map_err(|e| InstallError::Io(e.to_string()))?
    };
    let mut reader = resp.body_mut().as_reader();
    let mut buf = [0u8; CHUNK_BYTES];
    loop {
        let n = reader.read(&mut buf).map_err(|e| InstallError::Io(e.to_string()))?;
        if n == 0 { break; }
        file.write_all(&buf[..n]).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    Ok(())
}

/// The logic behind `install`, parameterized by the data directory and the source URL so tests can
/// point both at an isolated temp directory and a fixture server instead of the real Hub.
fn install_from(data_dir: &Path, entry: &ModelEntry, url: &str) -> Result<InstallOutcome, InstallError> {
    let final_path = install_path_in(data_dir, entry);
    if final_path.is_file() && sha256_file(&final_path).ok().as_deref() == Some(entry.sha256) {
        return Ok(InstallOutcome::AlreadyInstalled);
    }
    let dir = models_dir_in(data_dir);
    create_private_dir(&dir).map_err(|e| InstallError::Io(e.to_string()))?;
    check_disk_space(free_bytes_at(&dir).unwrap_or(0))?;
    let temp = temp_path_in(data_dir, entry);
    stream_to(url, &temp)?;
    let got = sha256_file(&temp).map_err(|e| InstallError::Io(e.to_string()))?;
    if got != entry.sha256 {
        let _ = fs::remove_file(&temp);
        return Err(InstallError::HashMismatch { expected: entry.sha256.to_string(), got });
    }
    fs::rename(&temp, &final_path).map_err(|e| InstallError::Io(e.to_string()))?;
    Ok(InstallOutcome::Installed)
}

/// Downloads and verifies `entry`, or confirms it is already installed. Contacts the Hub only here;
/// `status` below never does.
pub fn install(entry: &ModelEntry) -> Result<InstallOutcome, InstallError> {
    install_from(&crate::paths::data_dir(), entry, &hub_url(entry))
}

#[derive(Debug, PartialEq, Eq)]
pub enum StatusReport {
    NotInstalled,
    Verified(String),
    HashMismatch { expected: String, got: String },
}

fn status_in(data_dir: &Path, entry: &ModelEntry) -> StatusReport {
    let path = install_path_in(data_dir, entry);
    if !path.is_file() { return StatusReport::NotInstalled; }
    match sha256_file(&path) {
        Ok(got) if got == entry.sha256 => StatusReport::Verified(got),
        Ok(got) => StatusReport::HashMismatch { expected: entry.sha256.to_string(), got },
        Err(_) => StatusReport::NotInstalled,
    }
}

/// Reads and hashes the local file only; never contacts the Hub, so `model status` is safe to run
/// as often as wanted.
pub fn status(entry: &ModelEntry) -> StatusReport {
    status_in(&crate::paths::data_dir(), entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;
    use tiny_http::{Header, Response, Server};

    /// A fixture HTTP server serving fixed `content` bytes, honoring a `Range: bytes=N-` request
    /// with a 206 and the tail of `content` unless `ignore_range` is set, in which case it always
    /// replies 200 with the whole body regardless of the header -- the "server does not support
    /// resume" case `stream_to` must recover from.
    fn fixture_server(content: &'static [u8], ignore_range: bool) -> (String, std::sync::Arc<AtomicU32>) {
        let server = Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let calls = std::sync::Arc::new(AtomicU32::new(0));
        let calls2 = calls.clone();
        thread::spawn(move || {
            for req in server.incoming_requests() {
                calls2.fetch_add(1, Ordering::SeqCst);
                let range = req.headers().iter().find(|h| h.field.as_str().as_str().eq_ignore_ascii_case("Range")).map(|h| h.value.as_str().to_string());
                let start = range.as_deref().and_then(|r| r.strip_prefix("bytes=")).and_then(|r| r.strip_suffix('-')).and_then(|n| n.parse::<usize>().ok());
                let (status, body): (u16, &[u8]) = match (start, ignore_range) {
                    (Some(n), false) if n <= content.len() => (206, &content[n..]),
                    _ => (200, content),
                };
                let mut resp = Response::from_data(body).with_status_code(status);
                if status == 206 {
                    resp.add_header(Header::from_bytes(&b"Content-Range"[..], format!("bytes {}-{}/{}", start.unwrap(), content.len() - 1, content.len()).as_bytes()).unwrap());
                }
                let _ = req.respond(resp);
            }
        });
        (format!("http://127.0.0.1:{port}"), calls)
    }

    fn entry_for(content: &[u8]) -> ModelEntry {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let sha256 = hex(&hasher.finalize());
        ModelEntry { id: "test", repo: "test/repo", commit: "deadbeef", file_name: "test.gguf", size_bytes: content.len() as u64, sha256: Box::leak(sha256.into_boxed_str()), licence: "Apache-2.0" }
    }

    /// A fresh, private temp directory per test (the role `data_dir()` plays in production), so
    /// none of these tests touch the real `~/.monitor-lizard`. A counter, not just a timestamp,
    /// guarantees distinct paths even when the clock's resolution is coarser than tests run in
    /// parallel at (this is what the first version of this helper got wrong: two tests racing on
    /// the same nanosecond got the same directory and intermittently cross-contaminated each other).
    fn tmp_data_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("model-store-test-{}-{nanos}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_fresh_download_matching_the_manifest_hash_is_installed_and_leaves_no_temp_file() {
        let content: &'static [u8] = b"the quick brown fox jumps over the lazy dog, many times over";
        let entry = entry_for(content);
        let (url, calls) = fixture_server(content, false);
        let dir = tmp_data_dir();
        let r = install_from(&dir, &entry, &url);
        assert!(matches!(r, Ok(InstallOutcome::Installed)), "{r:?}");
        assert_eq!(fs::read(install_path_in(&dir, &entry)).unwrap(), content);
        assert!(!temp_path_in(&dir, &entry).exists(), "the temp file must not survive a successful install");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_partial_temp_file_is_resumed_not_restarted_when_the_server_honors_range() {
        let content: &'static [u8] = b"0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz";
        let entry = entry_for(content);
        let half = content.len() / 2;
        let dir = tmp_data_dir();
        fs::create_dir_all(models_dir_in(&dir)).unwrap();
        fs::write(temp_path_in(&dir, &entry), &content[..half]).unwrap();
        let (url, calls) = fixture_server(content, false);
        let r = install_from(&dir, &entry, &url);
        assert!(matches!(r, Ok(InstallOutcome::Installed)), "{r:?}");
        assert_eq!(fs::read(install_path_in(&dir, &entry)).unwrap(), content);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "one resumed request, not a restart");
    }

    #[test]
    fn a_server_that_ignores_range_gets_a_clean_restart_not_duplicated_bytes() {
        let content: &'static [u8] = b"abcdefghijklmnopqrstuvwxyz0123456789 abcdefghijklmnopqrstuvwxyz0123456789";
        let entry = entry_for(content);
        let half = content.len() / 2;
        let dir = tmp_data_dir();
        fs::create_dir_all(models_dir_in(&dir)).unwrap();
        fs::write(temp_path_in(&dir, &entry), &content[..half]).unwrap();
        let (url, _calls) = fixture_server(content, true);
        let r = install_from(&dir, &entry, &url);
        assert!(matches!(r, Ok(InstallOutcome::Installed)), "{r:?}");
        assert_eq!(fs::read(install_path_in(&dir, &entry)).unwrap(), content, "no duplicated prefix from blindly appending to the stale partial file");
    }

    #[test]
    fn a_hash_mismatch_deletes_the_temp_file_and_never_creates_the_final_path() {
        let declared = entry_for(b"what the manifest expects");
        let wrong_content: &'static [u8] = b"not what the manifest expects, a different byte count too";
        let (url, _calls) = fixture_server(wrong_content, false);
        let dir = tmp_data_dir();
        let r = install_from(&dir, &declared, &url);
        assert!(matches!(r, Err(InstallError::HashMismatch { .. })), "{r:?}");
        assert!(!temp_path_in(&dir, &declared).exists());
        assert!(!install_path_in(&dir, &declared).exists());
    }

    #[test]
    fn a_correctly_hashed_file_already_at_the_final_path_is_not_re_downloaded() {
        let content: &'static [u8] = b"already installed, verified once, never fetched again";
        let entry = entry_for(content);
        let dir = tmp_data_dir();
        fs::create_dir_all(models_dir_in(&dir)).unwrap();
        fs::write(install_path_in(&dir, &entry), content).unwrap();
        // No server is started at all: any attempt to contact one would fail to connect, so a
        // network call here fails the test rather than silently passing.
        let r = install_from(&dir, &entry, "http://127.0.0.1:1");
        assert_eq!(r.ok(), Some(InstallOutcome::AlreadyInstalled));
    }

    #[test]
    fn disk_space_below_the_minimum_is_refused_with_the_numbers() {
        let r = check_disk_space(1_000_000_000);
        assert!(matches!(r, Err(InstallError::DiskSpace { free_bytes: 1_000_000_000, min_bytes: MIN_FREE_BYTES })));
        assert!(check_disk_space(MIN_FREE_BYTES).is_ok());
    }

    #[test]
    fn status_reports_not_installed_verified_and_mismatched_without_any_network_access() {
        let content: &'static [u8] = b"status checks a local file only";
        let entry = entry_for(content);
        let dir = tmp_data_dir();
        assert_eq!(status_in(&dir, &entry), StatusReport::NotInstalled);
        fs::create_dir_all(models_dir_in(&dir)).unwrap();
        fs::write(install_path_in(&dir, &entry), content).unwrap();
        assert_eq!(status_in(&dir, &entry), StatusReport::Verified(entry.sha256.to_string()));
        fs::write(install_path_in(&dir, &entry), b"tampered bytes").unwrap();
        assert!(matches!(status_in(&dir, &entry), StatusReport::HashMismatch { .. }));
    }

    #[test]
    fn the_manifest_entries_are_distinct_and_their_commits_look_like_real_git_shas() {
        let ids: std::collections::HashSet<_> = MANIFEST.iter().map(|m| m.id).collect();
        assert_eq!(ids.len(), MANIFEST.len(), "duplicate --model id in the manifest");
        for m in MANIFEST {
            assert_eq!(m.commit.len(), 40, "{}: commit should be a full 40-char SHA-1", m.id);
            assert_eq!(m.sha256.len(), 64, "{}: sha256 should be 64 hex characters", m.id);
            assert!(m.sha256.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{}: sha256 must be lower-case hex", m.id);
        }
        assert!(find(DEFAULT_MODEL).is_some());
    }
}
