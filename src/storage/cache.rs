//! The latest scan, kept so a request never waits for one, and the rule for when a request may
//! start a new one. The page that asks is untrusted (any local web page can send a request that
//! passes the Host check), so a request can start a scan at most once per `MIN_INTERVAL`.
use super::{Item, Storage};
use serde::Serialize;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The least time between two scans that requests start.
pub const MIN_INTERVAL: Duration = Duration::from_secs(60);

/// Total time for one background scan the server runs. Longer than the command line's 20 s because a
/// developer home takes more than that, and each run adds to what the last one learned. The 20% CPU
/// limit and the low priority stay.
pub const SERVER_TIME_BUDGET: Duration = Duration::from_secs(120);

pub type ScanFn = Arc<dyn Fn() -> Storage + Send + Sync>;

#[derive(Serialize)]
struct View<'a> {
    scanning: bool,
    /// Unix seconds before which `?fresh=1` returns the cached result without a new scan.
    next_scan_allowed_at: Option<u64>,
    #[serde(flatten)]
    result: Option<&'a Storage>,
}

pub fn to_json(result: Option<&Storage>, scanning: bool, pretty: bool) -> String {
    let v = View { scanning, next_scan_allowed_at: None, result };
    (if pretty { serde_json::to_string_pretty(&v) } else { serde_json::to_string(&v) }).unwrap_or_default()
}

#[derive(Default)]
struct State {
    result: Option<Storage>,
    scanning: bool,
    /// Counts finished scans, so a confirmation given for one scan's items cannot be used on another's.
    generation: u64,
    /// When the last scan started: the clock for the interval, and the same moment in unix seconds.
    last_started: Option<(Instant, u64)>,
}

pub struct Cache { state: Mutex<State>, interval: Duration }

impl Default for Cache {
    fn default() -> Self { Self::with_interval(MIN_INTERVAL) }
}

/// Clears `scanning` even if the scan panics, so one failure cannot stop every later scan.
struct ScanGuard(Arc<Cache>);
impl Drop for ScanGuard {
    fn drop(&mut self) { self.0.lock().scanning = false; }
}

/// Only the exact value `1` asks for a new scan: `fresh=true`, `fresh=11` and `fresh=` do not.
pub fn wants_fresh(query: &str) -> bool {
    query.split('&').find_map(|kv| kv.strip_prefix("fresh=")) == Some("1")
}

impl Cache {
    pub fn with_interval(interval: Duration) -> Self { Cache { state: Mutex::new(State::default()), interval } }

    /// One place that tolerates a poisoned lock: the state is plain data and stays usable.
    fn lock(&self) -> MutexGuard<'_, State> { self.state.lock().unwrap_or_else(|e| e.into_inner()) }

    /// The reply for `GET /api/storage?<query>`.
    pub fn respond(self: &Arc<Self>, query: &str, scan: &ScanFn) -> String { self.request(wants_fresh(query), scan) }

    /// The cached result as JSON. The first request, and `fresh`, start a scan on another thread unless
    /// one is running or one started less than the interval ago. The reply is always the previous result
    /// with `scanning` saying whether a scan is under way.
    pub fn request(self: &Arc<Self>, fresh: bool, scan: &ScanFn) -> String {
        let mut st = self.lock();
        let allowed = st.last_started.is_none_or(|(at, _)| at.elapsed() >= self.interval);
        let start = (fresh || st.result.is_none()) && !st.scanning && allowed;
        if start {
            st.scanning = true;
            let unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            st.last_started = Some((Instant::now(), unix));
        }
        let next = st.last_started.map(|(_, unix)| unix + self.interval.as_secs());
        let json = serde_json::to_string(&View { scanning: st.scanning, next_scan_allowed_at: next, result: st.result.as_ref() }).unwrap_or_default();
        drop(st);
        if start {
            let (cache, scan) = (self.clone(), scan.clone());
            std::thread::spawn(move || {
                let guard = ScanGuard(cache.clone());
                // A request for a fresh scan means everything is walked, not reused.
                super::set_full_rescan(fresh);
                let result = scan();
                { let mut st = cache.lock(); st.result = Some(result); st.generation += 1; }
                drop(guard);
            });
        }
        json
    }

    /// The items of the latest scan for these ids, in order. A removal takes ids and never a path from
    /// the page, so an id that no scan listed cannot name anything.
    pub fn find(&self, ids: &[String]) -> Result<Vec<Item>, String> { self.find_in_generation(ids).map(|(_, items)| items) }

    /// `find`, and which scan the items came from, read under one lock so the two always agree.
    pub fn find_in_generation(&self, ids: &[String]) -> Result<(u64, Vec<Item>), String> {
        let st = self.lock();
        let Some(result) = st.result.as_ref() else { return Err("no scan has finished yet; scan first".into()) };
        let items = ids.iter().map(|id| result.items.iter().find(|i| &i.id == id).cloned().ok_or_else(|| format!("id {id} is not in the latest scan"))).collect::<Result<Vec<_>, _>>()?;
        Ok((st.generation, items))
    }

    /// Replaces the stored items with these for the same ids, but only while the scan they came from
    /// is still the latest: a scan that finished meanwhile has newer rows, and the caller must preview
    /// again. The ids, labels and scan number a confirmation is bound to do not change, only the sizes.
    pub fn replace_items(&self, generation: u64, fresh: Vec<Item>) -> bool {
        let mut st = self.lock();
        if st.generation != generation { return false; }
        let Some(result) = st.result.as_mut() else { return false };
        for f in fresh { if let Some(slot) = result.items.iter_mut().find(|i| i.id == f.id) { *slot = f; } }
        true
    }

    #[cfg(test)]
    pub(crate) fn with_result(result: Storage) -> Self {
        let c = Cache::default();
        { let mut st = c.lock(); st.result = Some(result); st.generation = 1; }
        c
    }

    /// Replaces the result as a finished scan would.
    #[cfg(test)]
    pub(crate) fn replace_result(&self, result: Storage) { let mut st = self.lock(); st.result = Some(result); st.generation += 1; }

    #[cfg(test)]
    fn is_scanning(&self) -> bool { self.lock().scanning }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    fn fake(scanned_at: u64) -> Storage { Storage::fake(scanned_at) }

    fn wait_until_idle(cache: &Cache) {
        let until = Instant::now() + Duration::from_secs(20);
        while cache.is_scanning() {
            assert!(Instant::now() < until, "the scan did not finish within 20 s");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn json(s: &str) -> serde_json::Value { serde_json::from_str(s).unwrap() }

    /// A scan that blocks until the test lets it go, and counts how many times it started.
    fn gated(started: Arc<AtomicUsize>) -> (ScanFn, mpsc::Sender<()>) {
        let (release, gate) = mpsc::channel::<()>();
        let gate = Mutex::new(gate);
        let scan: ScanFn = Arc::new(move || {
            let n = started.fetch_add(1, Ordering::SeqCst) as u64 + 1;
            gate.lock().unwrap().recv().ok();
            fake(n)
        });
        (scan, release)
    }

    #[test]
    fn two_requests_at_once_start_one_scan_and_the_reply_does_not_wait_for_it() {
        let started = Arc::new(AtomicUsize::new(0));
        let (scan, release) = gated(started.clone());
        let cache = Arc::new(Cache::with_interval(Duration::ZERO));
        // The scan cannot finish until released, so these replies came back while it was running.
        let (a, b) = (json(&cache.request(false, &scan)), json(&cache.request(true, &scan)));
        for v in [&a, &b] {
            assert_eq!(v["scanning"], true);
            assert!(v.get("items").is_none(), "no previous result yet");
        }
        release.send(()).unwrap();
        wait_until_idle(&cache);
        assert_eq!(started.load(Ordering::SeqCst), 1);
        let v = json(&cache.request(false, &scan));
        assert_eq!((v["scanning"].clone(), v["scanned_at"].clone()), (false.into(), 1.into()), "a cached result starts no scan");
        // `fresh` returns the previous result while the new scan runs.
        let v = json(&cache.request(true, &scan));
        assert_eq!((v["scanning"].clone(), v["scanned_at"].clone()), (true.into(), 1.into()));
        release.send(()).unwrap();
        wait_until_idle(&cache);
        assert_eq!(started.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_request_inside_the_interval_gets_the_cached_result_and_when_to_ask_again() {
        let started = Arc::new(AtomicUsize::new(0));
        let (scan, release) = gated(started.clone());
        release.send(()).unwrap();
        let cache = Arc::new(Cache::with_interval(Duration::from_secs(3600)));
        cache.request(false, &scan);
        wait_until_idle(&cache);
        for _ in 0..5 {
            let v = json(&cache.request(true, &scan));
            assert_eq!((v["scanning"].clone(), v["scanned_at"].clone()), (false.into(), 1.into()));
            let at = v["next_scan_allowed_at"].as_u64().expect("says when a new scan is allowed");
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            assert!(at > now + 3500 && at <= now + 3600, "{at} vs {now}");
        }
        assert_eq!(started.load(Ordering::SeqCst), 1, "five fresh requests inside the window started no scan");
    }

    #[test]
    fn after_the_interval_a_fresh_request_starts_a_scan_again() {
        let started = Arc::new(AtomicUsize::new(0));
        let (scan, release) = gated(started.clone());
        release.send(()).unwrap();
        release.send(()).unwrap();
        let cache = Arc::new(Cache::with_interval(Duration::from_millis(50)));
        cache.request(false, &scan);
        wait_until_idle(&cache);
        std::thread::sleep(Duration::from_millis(80));
        cache.request(true, &scan);
        wait_until_idle(&cache);
        assert_eq!(started.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn only_the_exact_value_one_asks_for_a_fresh_scan() {
        for q in ["fresh=1", "a=b&fresh=1", "fresh=1&a=b"] { assert!(wants_fresh(q), "{q}"); }
        for q in ["", "fresh=", "fresh=0", "fresh=11", "fresh=true", "fresh=1x", "xfresh=1", "fresh", "fresh=%31", "fresh=0&fresh=1"] { assert!(!wants_fresh(q), "{q}"); }
    }

    #[test]
    fn the_route_reply_follows_the_query() {
        let started = Arc::new(AtomicUsize::new(0));
        let (scan, release) = gated(started.clone());
        release.send(()).unwrap();
        let cache = Arc::new(Cache::with_interval(Duration::ZERO));
        assert_eq!(json(&cache.respond("", &scan))["scanning"], true, "the first request starts the first scan");
        wait_until_idle(&cache);
        assert_eq!(json(&cache.respond("fresh=true", &scan))["scanning"], false, "not the exact value: no scan");
        release.send(()).unwrap();
        assert_eq!(json(&cache.respond("fresh=1", &scan))["scanning"], true);
        wait_until_idle(&cache);
        assert_eq!(started.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_panicking_scan_does_not_leave_scanning_stuck() {
        let scan: ScanFn = Arc::new(|| panic!("scan failed on purpose"));
        let cache = Arc::new(Cache::with_interval(Duration::ZERO));
        cache.request(false, &scan);
        wait_until_idle(&cache);
        assert_eq!(json(&cache.request(false, &scan))["scanning"], true, "a later request can start a scan again");
        wait_until_idle(&cache);
    }
}
