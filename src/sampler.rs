//! A background thread that reads CPU, memory and network metrics every 2 s through direct system
//! calls (no shelling out), keeps the last hour in a ring buffer, and serves new samples over
//! `/api/metrics`. Host-wide metrics only: GPU, disk, per-process data and charting read this
//! module's output rather than living in it.
use serde::Serialize;
use std::collections::VecDeque;
use std::ffi::CString;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const RING_CAPACITY: usize = 1800; // 1 h at 2 s
const INTERVAL: Duration = Duration::from_secs(2);

/// A single value that may be unreadable (say why, never show 0) or belong to a process we may not inspect.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Reading<T> {
    Value(T),
    Unavailable(&'static str),
    /// Not produced by any host-wide reading in this module; kept for a per-process reading that
    /// can belong to another user (ps/lsof can see it exists but not read its counters).
    #[allow(dead_code)]
    Restricted,
}
impl<T> Reading<T> {
    fn ok(v: T) -> Self { Reading::Value(v) }
    fn or(r: Result<T, &'static str>) -> Self { match r { Ok(v) => Reading::Value(v), Err(e) => Reading::Unavailable(e) } }
}
// A derived Default would need T: Default too; every T here has none in particular, so this is written by hand.
impl<T> Default for Reading<T> { fn default() -> Self { Reading::Unavailable("unset") } }

#[derive(Clone, Serialize, Debug, Default)]
pub struct CpuSample {
    pub total_pct: Option<Reading<f32>>, pub user_pct: Option<Reading<f32>>, pub system_pct: Option<Reading<f32>>,
    /// One entry per logical CPU, in the order the kernel returns them. Grouping by performance/efficiency
    /// cluster needs `hw.perflevelN.logicalcpu` plus an IORegistry walk to map cores to clusters (unverified
    /// ordering): left to whatever renders this, rather than assumed here.
    pub per_core_pct: Option<Reading<Vec<f32>>>,
    pub perf_cores: Reading<u32>, pub eff_cores: Reading<u32>,
}
#[derive(Clone, Serialize, Debug)]
pub struct MemorySample {
    pub app_bytes: Reading<u64>, pub wired_bytes: Reading<u64>, pub compressed_bytes: Reading<u64>,
    /// External (file-backed, reclaimable) pages; the closest analogue to Activity Monitor's "Cached Files".
    pub cached_bytes: Reading<u64>, pub total_bytes: Reading<u64>,
    pub swap_used_bytes: Reading<u64>, pub swap_total_bytes: Reading<u64>,
    /// Raw `kern.memorystatus_vm_pressure_level`: 1 normal, 2 warn, 4 critical.
    pub pressure_level: Reading<u8>,
    pub pressure_pct_available: Reading<u8>,
}
#[derive(Clone, Serialize, Debug)]
pub struct NetworkSample {
    pub down_bytes_per_sec: Option<Reading<f64>>, pub up_bytes_per_sec: Option<Reading<f64>>,
    /// Interfaces up with an address; `None` when the list could not be read. Zero means no connection.
    pub interfaces_up: Option<u32>,
}

#[derive(Clone, Serialize, Debug)]
pub struct GpuSample { pub device_pct: Reading<u8>, pub renderer_pct: Reading<u8>, pub tiler_pct: Reading<u8> }

#[derive(Clone, Serialize, Debug)]
pub struct DiskSample {
    pub read_bytes_per_sec: Option<Reading<f64>>, pub write_bytes_per_sec: Option<Reading<f64>>,
    pub read_ops_per_sec: Option<Reading<f64>>, pub write_ops_per_sec: Option<Reading<f64>>,
    pub capacity_bytes: Reading<u64>, pub capacity_used_bytes: Reading<u64>,
}

#[derive(Clone, Serialize, Debug)]
pub struct Sample {
    pub ts: u64,
    pub load1: Reading<f64>, pub load5: Reading<f64>, pub load15: Reading<f64>,
    pub cpu: CpuSample, pub memory: MemorySample, pub network: NetworkSample,
    pub gpu: GpuSample, pub disk: DiskSample,
    /// The monitor's own CPU use, so it can be shown it is not the top consumer.
    pub self_cpu_pct: Option<Reading<f32>>,
}

// ---------- raw readers (each is Err(reason) on failure, never a silent zero) ----------

/// One home for every `sysctlbyname` call: reads a plain-old-data value of whatever size `T` is.
/// `err` is a literal written at each call site, naming the sysctl, so a failure says which one.
/// SAFETY: sysctlbyname writes at most `size_of::<T>()` bytes into `val`, a zeroed value of exactly
/// that size; every `T` used here (u64, i32, `xsw_usage`) is a plain value type with no invariants
/// an all-zero bit pattern could violate.
fn sysctl_into<T: Copy>(name: &str, err: &'static str) -> Result<T, &'static str> {
    let c = CString::new(name).map_err(|_| "bad sysctl name")?;
    let mut val: T = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<T>();
    let rc = unsafe { libc::sysctlbyname(c.as_ptr(), &mut val as *mut _ as *mut libc::c_void, &mut len, std::ptr::null_mut(), 0) };
    if rc == 0 { Ok(val) } else { Err(err) }
}
fn sysctl_u64(name: &str, err: &'static str) -> Result<u64, &'static str> { sysctl_into(name, err) }
fn sysctl_i32(name: &str, err: &'static str) -> Result<i32, &'static str> { sysctl_into(name, err) }

fn load_average() -> Result<[f64; 3], &'static str> {
    let mut avg = [0f64; 3];
    // SAFETY: getloadavg writes at most `len` doubles into `avg`, a 3-element stack array.
    let n = unsafe { libc::getloadavg(avg.as_mut_ptr(), 3) };
    if n == 3 { Ok(avg) } else { Err("getloadavg failed") }
}

/// Per-core CPU ticks (user, system, idle, nice), freshly read from the kernel each call.
fn cpu_ticks() -> Result<Vec<[u32; 4]>, &'static str> {
    let mut count: libc::natural_t = 0;
    let mut info: libc::processor_info_array_t = std::ptr::null_mut();
    let mut info_count: libc::mach_msg_type_number_t = 0;
    // SAFETY: host_processor_info is a standard Mach host call; on success it allocates `info`
    // (info_count `integer_t`s) in our address space, which we must free with vm_deallocate below.
    // mach2 (the crate libc's deprecation notice points at) does not expose host_processor_info at all;
    // libc's own binding is the only public, maintained way to call it, so the warning is suppressed, not avoided.
    #[allow(deprecated)]
    let rc = unsafe { libc::host_processor_info(libc::mach_host_self(), libc::PROCESSOR_CPU_LOAD_INFO, &mut count, &mut info, &mut info_count) };
    if rc != libc::KERN_SUCCESS || info.is_null() { return Err("host_processor_info failed"); }
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        // SAFETY: each processor's record is CPU_STATE_MAX (4) consecutive i32s starting at i*4;
        // `info_count` (checked above) covers `count * CPU_STATE_MAX` elements.
        let base = unsafe { info.add(i * libc::CPU_STATE_MAX as usize) };
        let ticks = unsafe { [*base as u32, *base.add(1) as u32, *base.add(2) as u32, *base.add(3) as u32] };
        out.push(ticks);
    }
    #[allow(deprecated)]
    unsafe { libc::vm_deallocate(libc::mach_task_self(), info as libc::vm_address_t, (info_count as usize * std::mem::size_of::<libc::integer_t>()) as libc::vm_size_t) };
    Ok(out)
}

// SAFETY: vm_statistics64 is a plain value type (no pointers/invariants); an all-zero bit pattern is valid,
// and host_statistics64 below overwrites it before it is read.
fn vm_stats() -> Result<libc::vm_statistics64, &'static str> {
    let mut stats: libc::vm_statistics64 = unsafe { std::mem::zeroed() };
    let mut count = (std::mem::size_of::<libc::vm_statistics64>() / std::mem::size_of::<libc::integer_t>()) as libc::mach_msg_type_number_t;
    // SAFETY: host_statistics64 fills exactly `count` integer_t's worth of `stats`, sized to match.
    // Same as above: mach2 does not expose host_statistics64 either.
    #[allow(deprecated)]
    let rc = unsafe { libc::host_statistics64(libc::mach_host_self(), libc::HOST_VM_INFO64, &mut stats as *mut _ as libc::host_info64_t, &mut count) };
    if rc == libc::KERN_SUCCESS { Ok(stats) } else { Err("host_statistics64 failed") }
}

/// Total received/sent bytes across all non-loopback interfaces, summed from `getifaddrs`.
// getifaddrs's AF_LINK entries carry the older 32-bit `struct if_data`, not `if_data64` --
// using it as a 64-bit struct reads past the real fields (confirmed against real traffic:
// the misread "byte count" was actually within a second of this machine's last interface
// change, i.e. `ifi_lastchange`, not `ifi_obytes`). `netstat -ib`'s own counters come from
// the routing socket's NET_RT_IFLIST2, which embeds if_data64 directly; this does the same.
fn net_bytes() -> Result<(u64, u64), &'static str> {
    let mib = [libc::CTL_NET, libc::PF_ROUTE, 0, 0, libc::NET_RT_IFLIST2, 0];
    let mut len: usize = 0;
    // SAFETY: a null oldp just asks sysctl to report the needed buffer size in oldlenp.
    if unsafe { libc::sysctl(mib.as_ptr() as *mut _, mib.len() as u32, std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) } != 0 {
        return Err("sysctl(NET_RT_IFLIST2) size query failed");
    }
    let mut buf = vec![0u8; len];
    // SAFETY: buf is sized by the query above; sysctl writes at most `len` bytes into it.
    if unsafe { libc::sysctl(mib.as_ptr() as *mut _, mib.len() as u32, buf.as_mut_ptr() as *mut _, &mut len, std::ptr::null_mut(), 0) } != 0 {
        return Err("sysctl(NET_RT_IFLIST2) read failed");
    }
    let (mut down, mut up) = (0u64, 0u64);
    let mut off = 0usize;
    while off + std::mem::size_of::<libc::if_msghdr2>() <= len {
        // SAFETY: each record's own ifm_msglen keeps us inside `buf`, checked by the loop guard.
        let hdr = unsafe { &*(buf.as_ptr().add(off) as *const libc::if_msghdr2) };
        if hdr.ifm_msglen == 0 { break; }
        if hdr.ifm_type as i32 == libc::RTM_IFINFO2 {
            let mut name_buf = [0u8; libc::IFNAMSIZ];
            // SAFETY: if_indextoname writes a NUL-terminated name into a buffer this size, or leaves it untouched on failure.
            let named = !unsafe { libc::if_indextoname(hdr.ifm_index as u32, name_buf.as_mut_ptr() as *mut i8) }.is_null();
            let name = named.then(|| unsafe { std::ffi::CStr::from_ptr(name_buf.as_ptr() as *const i8) }.to_string_lossy().into_owned());
            if name.as_deref() != Some("lo0") {
                down = down.saturating_add(hdr.ifm_data.ifi_ibytes);
                up = up.saturating_add(hdr.ifm_data.ifi_obytes);
            }
        }
        off += hdr.ifm_msglen as usize;
    }
    Ok((down, up))
}

// SAFETY: rusage is a plain value type, valid zeroed; getrusage(RUSAGE_SELF, ...) writes into it and
// cannot write past it (`ru` is sized exactly for the struct it expects).
/// Our own CPU use since the last tick, as a percentage of one core (matching `cpu.total_pct`'s
/// own-process equivalent): `None` on the first tick, same as the host-wide CPU rates.
fn self_cpu_pct(now: &Result<(i64, i64), &'static str>, prev: Option<&Prev>, elapsed: Option<f64>) -> Option<Reading<f32>> {
    match (now, prev, elapsed) {
        (Ok((u, s)), Some(p), Some(e)) => {
            let (u, s) = (*u, *s);
            let delta_micros = (u + s - p.self_micros).max(0) as f64;
            Some(Reading::ok(((delta_micros / 1e6) / e * 100.0) as f32))
        }
        (Err(reason), _, _) => Some(Reading::Unavailable(reason)),
        _ => None,
    }
}

fn self_cpu_ticks() -> Result<(i64, i64), &'static str> {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) } != 0 { return Err("getrusage failed"); }
    let micros = |tv: libc::timeval| tv.tv_sec as i64 * 1_000_000 + tv.tv_usec as i64;
    Ok((micros(ru.ru_utime), micros(ru.ru_stime)))
}

// ---------- the sampler: holds the previous tick so rates can be computed ----------

struct Prev { at: Instant, cpu_ticks: Vec<[u32; 4]>, net: (u64, u64), self_micros: i64, disk: Option<(u64, u64, u64, u64)> }

pub struct Sampler { prev: Option<Prev>, gpu: Option<crate::iokit::Gpu>, disks: crate::iokit::Disks }
impl Default for Sampler {
    fn default() -> Self { Self { prev: None, gpu: crate::iokit::Gpu::open(), disks: crate::iokit::Disks::open() } }
}

impl Sampler {
    /// One reading. The first call after startup (or after a counter goes backwards) has no rates,
    /// never a zero rate, matching the "first tick" and "counter resets" edge cases.
    pub fn tick(&mut self) -> Sample {
        let now = Instant::now();
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

        let ticks_now = cpu_ticks();
        let net_now = net_bytes();
        let self_now = self_cpu_ticks();
        let elapsed = self.prev.as_ref().map(|p| now.duration_since(p.at).as_secs_f64()).filter(|e| *e > 0.0);

        let cpu = build_cpu(&ticks_now, self.prev.as_ref().map(|p| &p.cpu_ticks), elapsed);
        let mut network = build_network(&net_now, self.prev.as_ref().map(|p| p.net), elapsed);
        network.interfaces_up = crate::netif::up_count();
        let self_cpu_pct = self_cpu_pct(&self_now, self.prev.as_ref(), elapsed);

        let gpu = build_gpu(self.gpu.as_ref().and_then(|g| g.utilization()));
        let disk_now = self.disks.boot_volume_stats();
        let disk = build_disk(disk_now.clone(), self.prev.as_ref().and_then(|p| p.disk), elapsed, crate::iokit::boot_volume_capacity());

        let load = load_average();
        let vm = vm_stats();
        let page = sysctl_u64("hw.pagesize", "sysctl hw.pagesize failed").unwrap_or(4096);
        let swap: Result<(u64, u64), &'static str> = sysctl_struct_swap();
        let perf_cores = sysctl_i32("hw.perflevel0.logicalcpu", "sysctl hw.perflevel0.logicalcpu failed").map(|v| v as u32);
        let eff_cores = sysctl_i32("hw.perflevel1.logicalcpu", "sysctl hw.perflevel1.logicalcpu failed").map(|v| v as u32);

        let memory = MemorySample {
            // "App memory": active + inactive + speculative, minus what's already counted elsewhere
            // (file-backed/cached, purgeable). Not internal_page_count minus compressor_page_count: on
            // this Mac compressor_page_count exceeds internal_page_count, so that subtraction saturates
            // to 0 every time -- a real bug caught by running this against live data, not by the unit
            // tests. Cross-checked against `vm_stat`'s own "Anonymous pages" line (within 0.1%); whether
            // this matches Activity Monitor's "App Memory" label exactly is still unverified (noted risk).
            app_bytes: Reading::or(vm.as_ref().map(|v| ((v.active_count as u64 + v.inactive_count as u64 + v.speculative_count as u64).saturating_sub(v.purgeable_count as u64).saturating_sub(v.external_page_count as u64)) * page).map_err(|e| *e)),
            wired_bytes: Reading::or(vm.as_ref().map(|v| v.wire_count as u64 * page).map_err(|e| *e)),
            compressed_bytes: Reading::or(vm.as_ref().map(|v| v.compressor_page_count as u64 * page).map_err(|e| *e)),
            cached_bytes: Reading::or(vm.as_ref().map(|v| v.external_page_count as u64 * page).map_err(|e| *e)),
            total_bytes: Reading::or(sysctl_u64("hw.memsize", "sysctl hw.memsize failed")),
            swap_used_bytes: Reading::or(swap.map(|s| s.0)),
            swap_total_bytes: Reading::or(swap.map(|s| s.1)),
            pressure_level: Reading::or(sysctl_i32("kern.memorystatus_vm_pressure_level", "sysctl kern.memorystatus_vm_pressure_level failed").map(|v| v as u8)),
            pressure_pct_available: Reading::or(sysctl_i32("kern.memorystatus_level", "sysctl kern.memorystatus_level failed").map(|v| v as u8)),
        };

        self.prev = Some(Prev {
            at: now,
            cpu_ticks: ticks_now.unwrap_or_default(),
            net: net_now.unwrap_or((0, 0)),
            self_micros: self_now.map(|(u, s)| u + s).unwrap_or(0),
            disk: disk_now.ok(),
        });

        Sample {
            ts,
            load1: Reading::or(load.map(|l| l[0])), load5: Reading::or(load.map(|l| l[1])), load15: Reading::or(load.map(|l| l[2])),
            cpu: CpuSample { perf_cores: Reading::or(perf_cores), eff_cores: Reading::or(eff_cores), ..cpu },
            memory, network, gpu, disk, self_cpu_pct,
        }
    }
}

/// `util` is `None` if there is no `IOAccelerator` entry at all (a VM, or an unsupported chip);
/// each inner `None` is one missing key within an entry that does exist.
fn build_gpu(util: Option<(Option<u8>, Option<u8>, Option<u8>)>) -> GpuSample {
    let reading = |v: Option<u8>, missing: &'static str| v.map(Reading::Value).unwrap_or(Reading::Unavailable(missing));
    match util {
        None => GpuSample {
            device_pct: Reading::Unavailable("no GPU registry entry"),
            renderer_pct: Reading::Unavailable("no GPU registry entry"),
            tiler_pct: Reading::Unavailable("no GPU registry entry"),
        },
        Some((d, r, t)) => GpuSample {
            device_pct: reading(d, "Device Utilization % missing from PerformanceStatistics"),
            renderer_pct: reading(r, "Renderer Utilization % missing from PerformanceStatistics"),
            tiler_pct: reading(t, "Tiler Utilization % missing from PerformanceStatistics"),
        },
    }
}

/// Diffs this tick's cumulative (read_bytes, write_bytes, read_ops, write_ops) against the last
/// tick's, the same wraparound-vs-reset reasoning `build_network` uses: `wrapping_sub` repairs a
/// real counter wrap, and an implausibly large delta for the elapsed time is treated as a reset
/// (the edge case "Counter reset -> Sample dropped") rather than a huge or negative rate. `now`
/// being `Err` (a `Statistics` key missing from every disk entry) reports the edge case "Disk
/// statistics key absent -> Unavailable, capacity still reported" directly, distinct from "no
/// previous tick yet", which reports `None` (no rate claimed either way) instead.
fn build_disk(now: Result<(u64, u64, u64, u64), &'static str>, prev: Option<(u64, u64, u64, u64)>, elapsed: Option<f64>, capacity: Option<(u64, u64)>) -> DiskSample {
    let capacity_bytes = Reading::or(capacity.map(|c| c.0).ok_or("statfs(\"/\") failed"));
    let capacity_used_bytes = Reading::or(capacity.map(|c| c.1).ok_or("statfs(\"/\") failed"));
    let now = match now {
        Ok(v) => v,
        Err(reason) => {
            return DiskSample {
                read_bytes_per_sec: Some(Reading::Unavailable(reason)), write_bytes_per_sec: Some(Reading::Unavailable(reason)),
                read_ops_per_sec: Some(Reading::Unavailable(reason)), write_ops_per_sec: Some(Reading::Unavailable(reason)),
                capacity_bytes, capacity_used_bytes,
            };
        }
    };
    let (rb, wb, ro, wo) = now;
    let (Some((prb, pwb, pro, pwo)), Some(e)) = (prev, elapsed) else {
        return DiskSample { read_bytes_per_sec: None, write_bytes_per_sec: None, read_ops_per_sec: None, write_ops_per_sec: None, capacity_bytes, capacity_used_bytes };
    };
    // Apple Silicon's internal NVMe can sustain multiple GB/s, so the bound is set far above any
    // real consumer SSD's throughput (tens of GB/s), not just above today's hardware: wide enough
    // to never misclassify genuine heavy I/O as a reset, while a real reset's wrapped delta would
    // still be many orders of magnitude past it.
    let plausible_max_bytes = (e * 50_000_000_000.0).max(50_000_000_000.0);
    let plausible_max_ops = (e * 10_000_000.0).max(10_000_000.0);
    let d_rb = rb.wrapping_sub(prb);
    let d_wb = wb.wrapping_sub(pwb);
    let d_ro = ro.wrapping_sub(pro);
    let d_wo = wo.wrapping_sub(pwo);
    if d_rb as f64 > plausible_max_bytes || d_wb as f64 > plausible_max_bytes || d_ro as f64 > plausible_max_ops || d_wo as f64 > plausible_max_ops {
        let reason = "disk counters went backwards";
        return DiskSample {
            read_bytes_per_sec: Some(Reading::Unavailable(reason)), write_bytes_per_sec: Some(Reading::Unavailable(reason)),
            read_ops_per_sec: Some(Reading::Unavailable(reason)), write_ops_per_sec: Some(Reading::Unavailable(reason)),
            capacity_bytes, capacity_used_bytes,
        };
    }
    DiskSample {
        read_bytes_per_sec: Some(Reading::ok(d_rb as f64 / e)), write_bytes_per_sec: Some(Reading::ok(d_wb as f64 / e)),
        read_ops_per_sec: Some(Reading::ok(d_ro as f64 / e)), write_ops_per_sec: Some(Reading::ok(d_wo as f64 / e)),
        capacity_bytes, capacity_used_bytes,
    }
}

fn sysctl_struct_swap() -> Result<(u64, u64), &'static str> {
    sysctl_into::<libc::xsw_usage>("vm.swapusage", "sysctl vm.swapusage failed").map(|v| (v.xsu_used, v.xsu_total))
}

/// Builds the CPU part of a sample from this tick's ticks and (if any) the previous tick's, diffing
/// per-core so a counter that goes backwards (a core coming online, a reset) is dropped for that core
/// rather than producing a nonsense or negative rate.
fn build_cpu(now: &Result<Vec<[u32; 4]>, &'static str>, prev: Option<&Vec<[u32; 4]>>, elapsed: Option<f64>) -> CpuSample {
    let now = match now { Ok(v) => v, Err(e) => return CpuSample { total_pct: Some(Reading::Unavailable(e)), user_pct: Some(Reading::Unavailable(e)), system_pct: Some(Reading::Unavailable(e)), per_core_pct: Some(Reading::Unavailable(e)), perf_cores: Reading::Unavailable("unset"), eff_cores: Reading::Unavailable("unset") } };
    let (Some(prev), Some(_)) = (prev, elapsed) else { return CpuSample::default() };
    if prev.len() != now.len() { return CpuSample::default(); }
    let mut per_core = Vec::with_capacity(now.len());
    let (mut tu, mut ts, mut tt) = (0u64, 0u64, 0u64);
    let mut any_backwards = false;
    // `wrapping_sub` repairs a real wrap (the counter reaching u32::MAX and rolling over), which
    // genuinely happens for a tick counter over a long uptime; it cannot repair a true reset (a core
    // coming back online, the kernel resetting the counter), which produces the same huge wrapped
    // value at the bit level. The two are told apart by plausibility: no real core accumulates more
    // than a generous multiple of the elapsed wall-clock time in ticks, at any plausible tick rate.
    let plausible_max = (elapsed.unwrap_or(1.0) * 10_000.0).max(10_000.0) as u64;
    for (n, p) in now.iter().zip(prev.iter()) {
        let d: Vec<u64> = n.iter().zip(p.iter()).map(|(&a, &b)| a.wrapping_sub(b) as u64).collect();
        let total: u64 = d.iter().sum();
        if total > plausible_max { any_backwards = true; per_core.push(0.0); continue; }
        let (u, s) = (d[libc::CPU_STATE_USER as usize] + d[libc::CPU_STATE_NICE as usize], d[libc::CPU_STATE_SYSTEM as usize]);
        tu += u; ts += s; tt += total.max(1);
        per_core.push(if total > 0 { ((u + s) as f64 / total as f64 * 100.0) as f32 } else { 0.0 });
    }
    if tt == 0 { return CpuSample::default(); }
    let pc = if any_backwards { Reading::Unavailable("a core's counters went backwards") } else { Reading::ok(per_core) };
    CpuSample {
        total_pct: Some(Reading::ok(((tu + ts) as f64 / tt as f64 * 100.0) as f32)),
        user_pct: Some(Reading::ok((tu as f64 / tt as f64 * 100.0) as f32)),
        system_pct: Some(Reading::ok((ts as f64 / tt as f64 * 100.0) as f32)),
        per_core_pct: Some(pc),
        perf_cores: Reading::Unavailable("unset"), eff_cores: Reading::Unavailable("unset"),
    }
}

fn build_network(now: &Result<(u64, u64), &'static str>, prev: Option<(u64, u64)>, elapsed: Option<f64>) -> NetworkSample {
    let &(dn, un) = match now { Ok(v) => v, Err(e) => return NetworkSample { down_bytes_per_sec: Some(Reading::Unavailable(e)), up_bytes_per_sec: Some(Reading::Unavailable(e)), interfaces_up: None } };
    let (Some((dp, up)), Some(e)) = (prev, elapsed) else { return NetworkSample { down_bytes_per_sec: None, up_bytes_per_sec: None, interfaces_up: None } };
    // u64 counters can wrap (rare at these widths, but cheap to handle correctly); a backwards jump
    // outside that (an interface reset) is dropped rather than shown as a huge or negative rate.
    let dd = dn.wrapping_sub(dp);
    let du = un.wrapping_sub(up);
    if dd > u64::MAX / 2 || du > u64::MAX / 2 {
        return NetworkSample { down_bytes_per_sec: Some(Reading::Unavailable("interface counters reset")), up_bytes_per_sec: Some(Reading::Unavailable("interface counters reset")), interfaces_up: None };
    }
    NetworkSample { down_bytes_per_sec: Some(Reading::ok(dd as f64 / e)), up_bytes_per_sec: Some(Reading::ok(du as f64 / e)), interfaces_up: None }
}

// ---------- ring buffer and the background thread ----------

pub type Ring = Arc<RwLock<VecDeque<Sample>>>;

pub fn start() -> Ring {
    let ring: Ring = Arc::new(RwLock::new(VecDeque::with_capacity(RING_CAPACITY)));
    let r = ring.clone();
    std::thread::spawn(move || {
        let mut sampler = Sampler::default();
        loop {
            let sample = sampler.tick();
            if let Ok(mut buf) = r.write() {
                if buf.len() >= RING_CAPACITY { buf.pop_front(); }
                buf.push_back(sample);
            }
            std::thread::sleep(INTERVAL);
        }
    });
    ring
}

pub fn since(ring: &Ring, ts: u64) -> Vec<Sample> {
    ring.read().map(|b| b.iter().filter(|s| s.ts > ts).cloned().collect()).unwrap_or_default()
}

/// One reading, taken twice ~250 ms apart so the CLI can show a rate without waiting for the thread.
pub fn one_shot() -> Sample {
    let mut s = Sampler::default();
    let _ = s.tick();
    std::thread::sleep(Duration::from_millis(250));
    s.tick()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_sysctl_name_is_unavailable_with_the_given_reason_not_a_panic() {
        let r: Result<u64, &'static str> = sysctl_u64("sm.no.such.sysctl.exists", "sysctl sm.no.such.sysctl.exists failed");
        assert_eq!(r, Err("sysctl sm.no.such.sysctl.exists failed"));
    }

    #[test]
    fn first_tick_has_no_rates_not_zero() {
        let now: Result<Vec<[u32; 4]>, &'static str> = Ok(vec![[100, 50, 1000, 0]]);
        let cpu = build_cpu(&now, None, None);
        assert_eq!(cpu.total_pct, None, "no previous tick: absent, not Some(0)");
    }

    #[test]
    fn cpu_percent_from_two_ticks() {
        let prev = vec![[100, 50, 1000, 0]];
        let now: Result<Vec<[u32; 4]>, &'static str> = Ok(vec![[120, 60, 1020, 0]]); // +20 user, +10 sys, +20 idle = 50 total, 30 busy
        let cpu = build_cpu(&now, Some(&prev), Some(1.0));
        let Some(Reading::Value(pct)) = cpu.total_pct else { panic!("expected a value") };
        assert!((pct - 60.0).abs() < 0.01, "{pct}");
    }

    #[test]
    fn a_core_going_backwards_is_dropped_not_negative() {
        // Two cores: the first resets (a real reset, not a u32 wrap, so the delta is implausibly huge
        // even after wrapping_sub); the second ticks normally, so the aggregate is still computable.
        let prev = vec![[1000, 500, 2000, 0], [100, 50, 1000, 0]];
        let now: Result<Vec<[u32; 4]>, &'static str> = Ok(vec![[10, 5, 20, 0], [120, 60, 1020, 0]]);
        let cpu = build_cpu(&now, Some(&prev), Some(1.0));
        assert_eq!(cpu.per_core_pct, Some(Reading::Unavailable("a core's counters went backwards")));
        // The aggregate total_pct is still reported, from the one core that is readable.
        assert!(matches!(cpu.total_pct, Some(Reading::Value(_))), "{:?}", cpu.total_pct);
    }

    #[test]
    fn mismatched_core_count_is_unavailable_not_a_panic() {
        let prev = vec![[1, 1, 1, 1]];
        let now: Result<Vec<[u32; 4]>, &'static str> = Ok(vec![[1, 1, 1, 1], [1, 1, 1, 1]]); // a core appeared
        let cpu = build_cpu(&now, Some(&prev), Some(1.0));
        assert_eq!(cpu.total_pct, None);
    }

    #[test]
    fn network_rate_from_two_ticks() {
        let now: Result<(u64, u64), &'static str> = Ok((2_000, 1_000));
        let net = build_network(&now, Some((1_000, 500)), Some(2.0));
        assert_eq!(net.down_bytes_per_sec, Some(Reading::Value(500.0)));
        assert_eq!(net.up_bytes_per_sec, Some(Reading::Value(250.0)));
    }

    #[test]
    fn network_counter_wraparound_gives_the_right_delta() {
        // u64 wrap is realistically never hit, but wrapping_sub must still do the right thing.
        let now: Result<(u64, u64), &'static str> = Ok((50, 0));
        let net = build_network(&now, Some((u64::MAX - 49, 0)), Some(1.0));
        assert_eq!(net.down_bytes_per_sec, Some(Reading::Value(100.0)));
    }

    #[test]
    fn network_interface_reset_is_unavailable_not_a_huge_number() {
        let now: Result<(u64, u64), &'static str> = Ok((10, 10));
        let net = build_network(&now, Some((5_000_000_000, 0)), Some(1.0));
        assert_eq!(net.down_bytes_per_sec, Some(Reading::Unavailable("interface counters reset")));
    }

    #[test]
    fn gpu_with_no_registry_entry_is_unavailable_with_the_edge_case_reason() {
        let gpu = build_gpu(None);
        assert_eq!(gpu.device_pct, Reading::Unavailable("no GPU registry entry"));
        assert_eq!(gpu.renderer_pct, Reading::Unavailable("no GPU registry entry"));
        assert_eq!(gpu.tiler_pct, Reading::Unavailable("no GPU registry entry"));
    }

    #[test]
    fn gpu_with_a_missing_key_is_unavailable_for_just_that_key() {
        // The entry exists (Some(..)) but one key -- Tiler Utilization % -- wasn't in the dictionary,
        // which some chips and driver versions genuinely omit even when the entry itself is present.
        let gpu = build_gpu(Some((Some(12), Some(8), None)));
        assert_eq!(gpu.device_pct, Reading::Value(12));
        assert_eq!(gpu.renderer_pct, Reading::Value(8));
        assert!(matches!(gpu.tiler_pct, Reading::Unavailable(_)));
    }

    #[test]
    fn disk_statistics_key_absent_is_unavailable_not_a_silent_zero_and_capacity_is_still_reported() {
        // The exact edge case: "Disk statistics key absent -> Unavailable, capacity still reported."
        let disk = build_disk(Err("no IOBlockStorageDriver entry has a complete Statistics dictionary"), None, None, Some((1_000_000, 400_000)));
        assert_eq!(disk.read_bytes_per_sec, Some(Reading::Unavailable("no IOBlockStorageDriver entry has a complete Statistics dictionary")));
        assert_eq!(disk.write_ops_per_sec, Some(Reading::Unavailable("no IOBlockStorageDriver entry has a complete Statistics dictionary")));
        assert_eq!(disk.capacity_bytes, Reading::Value(1_000_000), "capacity is independent of whether disk rate stats are available");
    }

    #[test]
    fn disk_first_tick_has_no_rates_but_still_reports_capacity() {
        let disk = build_disk(Ok((100, 50, 10, 5)), None, None, Some((1_000_000, 400_000)));
        assert!(disk.read_bytes_per_sec.is_none() && disk.write_ops_per_sec.is_none());
        assert_eq!(disk.capacity_bytes, Reading::Value(1_000_000));
        assert_eq!(disk.capacity_used_bytes, Reading::Value(400_000));
    }

    #[test]
    fn disk_rate_from_two_ticks() {
        let disk = build_disk(Ok((1100, 600, 20, 15)), Some((100, 50, 10, 5)), Some(1.0), None);
        assert_eq!(disk.read_bytes_per_sec, Some(Reading::Value(1000.0)));
        assert_eq!(disk.write_bytes_per_sec, Some(Reading::Value(550.0)));
        assert_eq!(disk.read_ops_per_sec, Some(Reading::Value(10.0)));
        assert_eq!(disk.write_ops_per_sec, Some(Reading::Value(10.0)));
        assert!(matches!(disk.capacity_bytes, Reading::Unavailable(_)), "statfs failing is independent of the rate calculation");
    }

    #[test]
    fn disk_counter_reset_is_unavailable_not_a_huge_number() {
        // Lifetime counters go backwards: a real reset, not a wrap (far beyond any plausible
        // multi-GB/s sustained rate for the 1 s elapsed).
        let disk = build_disk(Ok((10, 10, 1, 1)), Some((5_000_000_000_000, 0, 0, 0)), Some(1.0), None);
        assert_eq!(disk.read_bytes_per_sec, Some(Reading::Unavailable("disk counters went backwards")));
        assert_eq!(disk.write_ops_per_sec, Some(Reading::Unavailable("disk counters went backwards")));
    }

    /// A minimal Sample for ring-buffer tests that only care about `ts`; every reading is
    /// Unavailable/None, which is fine since nothing here inspects them.
    fn sample(ts: u64) -> Sample {
        Sample {
            ts, load1: Reading::Unavailable("x"), load5: Reading::Unavailable("x"), load15: Reading::Unavailable("x"),
            cpu: CpuSample::default(),
            memory: MemorySample { app_bytes: Reading::Unavailable("x"), wired_bytes: Reading::Unavailable("x"), compressed_bytes: Reading::Unavailable("x"), cached_bytes: Reading::Unavailable("x"), total_bytes: Reading::Unavailable("x"), swap_used_bytes: Reading::Unavailable("x"), swap_total_bytes: Reading::Unavailable("x"), pressure_level: Reading::Unavailable("x"), pressure_pct_available: Reading::Unavailable("x") },
            network: NetworkSample { down_bytes_per_sec: None, up_bytes_per_sec: None, interfaces_up: None },
            gpu: GpuSample { device_pct: Reading::Unavailable("x"), renderer_pct: Reading::Unavailable("x"), tiler_pct: Reading::Unavailable("x") },
            disk: DiskSample { read_bytes_per_sec: None, write_bytes_per_sec: None, read_ops_per_sec: None, write_ops_per_sec: None, capacity_bytes: Reading::Unavailable("x"), capacity_used_bytes: Reading::Unavailable("x") },
            self_cpu_pct: None,
        }
    }

    #[test]
    fn ring_buffer_drops_the_oldest_when_full() {
        let ring: Ring = Arc::new(RwLock::new(VecDeque::with_capacity(3)));
        for ts in 1..=4u64 {
            let mut buf = ring.write().unwrap();
            if buf.len() >= 3 { buf.pop_front(); }
            buf.push_back(sample(ts));
        }
        let buf = ring.read().unwrap();
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.iter().map(|s| s.ts).collect::<Vec<_>>(), vec![2, 3, 4]);
    }

    #[test]
    fn since_returns_only_newer_samples() {
        let ring: Ring = Arc::new(RwLock::new(VecDeque::new()));
        for ts in [10u64, 20, 30] {
            ring.write().unwrap().push_back(sample(ts));
        }
        assert_eq!(since(&ring, 20).iter().map(|s| s.ts).collect::<Vec<_>>(), vec![30]);
        assert!(since(&ring, 1000).is_empty(), "since newer than every sample is empty");
    }

    // Smoke test only: reads real system state, so it is not a correctness proof, just "does not panic
    // or return garbage on this machine."
    #[test]
    fn macos_smoke_two_real_ticks_give_a_plausible_reading() {
        let mut s = Sampler::default();
        let _ = s.tick();
        std::thread::sleep(Duration::from_millis(50));
        let sample = s.tick();
        if let Some(Reading::Value(pct)) = sample.cpu.total_pct { assert!((0.0..=100.0 * 16.0).contains(&pct), "{pct}"); }
        if let Reading::Value(total) = sample.memory.total_bytes { assert!(total > 1 << 30, "hw.memsize implausibly small: {total}"); }
    }
}

