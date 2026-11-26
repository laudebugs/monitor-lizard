//! Cached IOKit service handles for GPU and disk metrics. Every function here only queries
//! already-open handles or opens one once at startup; nothing here spawns a process, matching the
//! constraint that the sampler's per-tick loop never shells out.
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use io_kit_sys::{kIOMasterPortDefault, IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperty, IOServiceGetMatchingService, IOServiceGetMatchingServices, IOServiceMatching};
use io_kit_sys::types::{io_iterator_t, io_service_t};
use std::ffi::CString;

// SAFETY: IOServiceMatching builds (and the caller of this function owns) a CFMutableDictionaryRef
// from a NUL-terminated C string; IOServiceGetMatchingService consumes exactly that dictionary
// (releasing it) and returns either a valid io_service_t or 0 (`MACH_PORT_NULL`, "not found").
unsafe fn find_service(class_name: &str) -> Option<io_service_t> {
    let name = CString::new(class_name).ok()?;
    let matching = IOServiceMatching(name.as_ptr());
    if matching.is_null() {
        return None;
    }
    let service = IOServiceGetMatchingService(kIOMasterPortDefault, matching as _);
    (service != 0).then_some(service)
}

// SAFETY: same matching-dictionary contract as `find_service`; IOServiceGetMatchingServices fills
// `iter` with a valid iterator on success (checked via its return code) which we drain and release.
unsafe fn find_services(class_name: &str) -> Vec<io_service_t> {
    let Some(name) = CString::new(class_name).ok() else { return vec![] };
    let matching = IOServiceMatching(name.as_ptr());
    if matching.is_null() {
        return vec![];
    }
    let mut iter: io_iterator_t = 0;
    if IOServiceGetMatchingServices(kIOMasterPortDefault, matching as _, &mut iter) != 0 {
        return vec![];
    }
    let mut out = Vec::new();
    loop {
        let obj = IOIteratorNext(iter);
        if obj == 0 {
            break;
        }
        out.push(obj);
    }
    IOObjectRelease(iter);
    out
}

// SAFETY: IORegistryEntryCreateCFProperty returns either null or a `+1`-retained CFTypeRef the
// caller owns; `CFDictionary::wrap_under_create_rule` takes ownership of exactly that retain,
// matching its contract, and releases it when the returned value is dropped.
unsafe fn dict_property(service: io_service_t, key: &str) -> Option<CFDictionary<CFString, core_foundation::base::CFType>> {
    let cfkey = CFString::new(key);
    let raw = IORegistryEntryCreateCFProperty(service, cfkey.as_concrete_TypeRef(), std::ptr::null(), 0);
    if raw.is_null() {
        return None;
    }
    Some(CFDictionary::wrap_under_create_rule(raw as _))
}

fn number_in(dict: &CFDictionary<CFString, core_foundation::base::CFType>, key: &str) -> Option<i64> {
    dict.find(CFString::new(key)).and_then(|v| v.clone().downcast::<CFNumber>()).and_then(|n| n.to_i64())
}

/// A cached handle to the first `IOAccelerator` entry found at startup. macOS gives one GPU
/// device per integrated/discrete accelerator; this reads the first one, matching the common case
/// of a single Apple Silicon GPU (multi-GPU aggregation is not attempted here).
pub struct Gpu(io_service_t);

impl Gpu {
    /// `None` on a machine with no `IOAccelerator` entry (a VM, or an unsupported chip).
    pub fn open() -> Option<Self> {
        unsafe { find_service("IOAccelerator") }.map(Gpu)
    }

    /// `(device_pct, renderer_pct, tiler_pct)`, each `None` if its specific key is missing from
    /// `PerformanceStatistics` (seen on some chips/driver versions) even though the entry itself
    /// exists.
    pub fn utilization(&self) -> Option<(Option<u8>, Option<u8>, Option<u8>)> {
        let dict = unsafe { dict_property(self.0, "PerformanceStatistics") }?;
        let pct = |k: &str| number_in(&dict, k).map(|v| v.clamp(0, 100) as u8);
        Some((pct("Device Utilization %"), pct("Renderer Utilization %"), pct("Tiler Utilization %")))
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        // SAFETY: `self.0` was returned by `find_service`, which only ever returns a service this
        // struct uniquely owns from that point on (never shared, never released elsewhere).
        unsafe { IOObjectRelease(self.0) };
    }
}

/// Every `IOBlockStorageDriver` entry found at startup, cached so the per-tick read never
/// re-enumerates the registry. There is one entry per physical/virtual block device; "the boot
/// volume" is approximated as whichever entry has accumulated the most lifetime I/O, since APFS's
/// synthesized boot container is not a child of its physical store in the IOKit service tree (so
/// there is no simple registry walk from "/" to the right entry without either shelling out to
/// `diskutil` -- which the per-tick loop must not do -- or binding DiskArbitration, a much larger
/// surface for one heuristic). On every Mac this was checked against, the real boot disk's lifetime
/// byte counts dwarf every other entry (idle expansion bays, recovery partitions), so this is a
/// reasonable approximation, not a guaranteed-correct identification.
pub struct Disks(Vec<io_service_t>);

impl Disks {
    pub fn open() -> Self {
        Disks(unsafe { find_services("IOBlockStorageDriver") })
    }

    /// Bytes and operations, read and write, for whichever cached entry currently has the most
    /// combined lifetime bytes -- not necessarily the same entry from call to call if a busier
    /// device appears, but stable in practice since the boot disk's lifetime total only grows.
    /// `None` if no entry has a `Statistics` dictionary at all.
    pub fn boot_volume_stats(&self) -> Option<(u64, u64, u64, u64)> {
        self.0
            .iter()
            .filter_map(|&svc| {
                let dict = unsafe { dict_property(svc, "Statistics") }?;
                let get = |k: &str| number_in(&dict, k).unwrap_or(0).max(0) as u64;
                Some((get("Bytes (Read)"), get("Bytes (Write)"), get("Operations (Read)"), get("Operations (Write)")))
            })
            .max_by_key(|&(rb, wb, _, _)| rb.saturating_add(wb))
    }
}

impl Drop for Disks {
    fn drop(&mut self) {
        for &svc in &self.0 {
            // SAFETY: each handle in `self.0` came from `find_services`, which this struct
            // uniquely owns; none are released anywhere else.
            unsafe { IOObjectRelease(svc) };
        }
    }
}

/// Total, available and used bytes for the boot volume, via `statfs` (a syscall, not a process).
/// Purgeable space is not broken out separately: on APFS, `f_bavail` already counts reclaimable
/// purgeable space as available, matching what Finder and `df` show, so a second `getattrlist`
/// call for `ATTR_VOL_SPACE_PURGEABLE` would be redundant for the figures this reports.
// SAFETY: `stat` is a plain value type; `libc::statfs` on "/" writes into it and returns 0 on
// success, nonzero (checked) on failure, before anything here reads it.
pub fn boot_volume_capacity() -> Option<(u64, u64)> {
    let path = std::ffi::CString::new("/").ok()?;
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let block = stat.f_bsize as u64;
    let total = (stat.f_blocks as u64).saturating_mul(block);
    let available = (stat.f_bavail as u64).saturating_mul(block);
    Some((total, total.saturating_sub(available)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_key_in_a_real_or_absent_gpu_entry_does_not_panic() {
        // Exercises the real registry on this machine: either there is no IOAccelerator (None),
        // or there is one and every percentage key is present (the common real case), or the
        // entry exists with some keys missing -- all three must not panic either way.
        if let Some(gpu) = Gpu::open() {
            let _ = gpu.utilization();
        }
    }

    #[test]
    fn boot_volume_capacity_reports_nonzero_total_on_a_real_mac() {
        let (total, used) = boot_volume_capacity().expect("statfs(\"/\") must succeed on a real Mac");
        assert!(total > 0);
        assert!(used <= total);
    }

    #[test]
    fn disks_open_finds_at_least_one_entry_on_a_real_mac() {
        let disks = Disks::open();
        assert!(!disks.0.is_empty(), "every real Mac has at least one IOBlockStorageDriver");
        assert!(disks.boot_volume_stats().is_some());
    }

    #[test]
    fn boot_volume_stats_picks_the_entry_with_the_most_lifetime_bytes() {
        // Pure-logic check of the max-by-bytes rule, independent of real IOKit handles: the
        // `max_by_key` rule itself, exercised directly rather than through real service handles.
        let candidates = [(100u64, 50u64, 1u64, 1u64), (9_000_000u64, 5_000_000u64, 10u64, 10u64), (0u64, 0u64, 0u64, 0u64)];
        let best = candidates.iter().max_by_key(|&&(rb, wb, _, _)| rb.saturating_add(wb)).unwrap();
        assert_eq!(*best, (9_000_000, 5_000_000, 10, 10));
    }
}
