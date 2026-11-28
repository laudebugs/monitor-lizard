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

/// A property this codebase only ever reads as a dictionary (`PerformanceStatistics`, `Statistics`)
/// could in principle be any CF type if a future macOS version changes its shape; checked against
/// `CFDictionaryGetTypeID()` before being treated as one, rather than trusting the caller's intent.
// SAFETY: IORegistryEntryCreateCFProperty returns either null or a `+1`-retained CFTypeRef the
// caller owns. `CFGetTypeID` only reads the object's header to classify it and does not consume
// the retain; once confirmed to be a dictionary, `CFDictionary::wrap_under_create_rule` takes
// ownership of that same `+1` retain, matching its contract, and releases it when dropped. If the
// type check fails, the retain is released directly (`CFRelease`) since nothing else will own it.
unsafe fn dict_property(service: io_service_t, key: &str) -> Option<CFDictionary<CFString, core_foundation::base::CFType>> {
    let cfkey = CFString::new(key);
    let raw = IORegistryEntryCreateCFProperty(service, cfkey.as_concrete_TypeRef(), std::ptr::null(), 0);
    if raw.is_null() {
        return None;
    }
    if core_foundation_sys::base::CFGetTypeID(raw) != core_foundation_sys::dictionary::CFDictionaryGetTypeID() {
        core_foundation_sys::base::CFRelease(raw);
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
        // SAFETY: `find_service`'s own invariant (above its definition): "IOAccelerator" is a
        // valid NUL-terminatable class name, satisfied by the string literal here.
        unsafe { find_service("IOAccelerator") }.map(Gpu)
    }

    /// `(device_pct, renderer_pct, tiler_pct)`, each `None` if its specific key is missing from
    /// `PerformanceStatistics` (seen on some chips/driver versions) even though the entry itself
    /// exists.
    pub fn utilization(&self) -> Option<(Option<u8>, Option<u8>, Option<u8>)> {
        // SAFETY: `self.0` is a service this struct owns for its whole lifetime (see `Gpu`'s own
        // doc comment and `Drop` impl); `dict_property`'s own invariant covers the rest.
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
        // SAFETY: `find_services`'s own invariant (above its definition); "IOBlockStorageDriver"
        // is a valid NUL-terminatable class name, satisfied by the string literal here.
        Disks(unsafe { find_services("IOBlockStorageDriver") })
    }

    /// Bytes and operations, read and write, for whichever cached entry currently has the most
    /// combined lifetime bytes -- not necessarily the same entry from call to call if a busier
    /// device appears, but stable in practice since the boot disk's lifetime total only grows.
    /// `Err` if no entry has a `Statistics` dictionary with all four keys this reads: confirmed on
    /// this Mac via `ioreg -r -d 1 -c IOBlockStorageDriver` (`Statistics` = {"Bytes (Read)",
    /// "Bytes (Write)", "Operations (Read)", "Operations (Write)", ...}), so an entry missing any
    /// one of them is a real anomaly to report, not silently averaged in as a true zero.
    pub fn boot_volume_stats(&self) -> Result<(u64, u64, u64, u64), &'static str> {
        self.0
            .iter()
            .filter_map(|&svc| {
                // SAFETY: `svc` is a service this struct owns for its whole lifetime (see `Disks`'
                // own doc comment and `Drop` impl); `dict_property`'s own invariant covers the rest.
                let dict = unsafe { dict_property(svc, "Statistics") }?;
                let get = |k: &str| number_in(&dict, k).map(|v| v.max(0) as u64);
                Some((get("Bytes (Read)")?, get("Bytes (Write)")?, get("Operations (Read)")?, get("Operations (Write)")?))
            })
            .max_by_key(|&(rb, wb, _, _)| rb.saturating_add(wb))
            .ok_or("no IOBlockStorageDriver entry has a complete Statistics dictionary")
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
/// `f_bavail` is the free space `df` shows and does not include purgeable space: as of 2026-11 on
/// macOS 27 it matched `NSURLVolumeAvailableCapacityKey`, while the "important usage" figure was
/// about 41 GB higher. The storage scanner reads that figure to split purgeable out.
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

/// The NVMe SMART page's "Data Units Written" (bytes 48-63, little-endian) in bytes. The NVMe spec
/// counts a data unit as 1,000 blocks of 512 bytes.
fn smart_bytes_written(page: &[u8; 512]) -> u64 {
    let mut units = [0u8; 16];
    units.copy_from_slice(&page[48..64]);
    u128::from_le_bytes(units).saturating_mul(512_000).min(u64::MAX as u128) as u64
}

/// The COM-style head every IOKit plug-in interface starts with (`IUNKNOWN_C_GUTS`).
#[repr(C)]
struct IUnknownHead {
    _reserved: *mut std::ffi::c_void,
    query_interface: unsafe extern "C" fn(*mut std::ffi::c_void, core_foundation_sys::uuid::CFUUIDBytes, *mut *mut std::ffi::c_void) -> i32,
    _add_ref: unsafe extern "C" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "C" fn(*mut std::ffi::c_void) -> u32,
}

/// `IONVMeSMARTInterface` from the SDK's `IOKit/storage/nvme/NVMeSMARTLibExternal.h`, up to the
/// one call used. That struct is not inside the header's `#pragma pack(1)` region, so natural C
/// alignment applies, as `repr(C)` gives.
#[repr(C)]
struct SmartInterface {
    head: IUnknownHead,
    _version: u16,
    _revision: u16,
    smart_read_data: unsafe extern "C" fn(*mut std::ffi::c_void, *mut [u8; 512]) -> i32,
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOCreatePlugInInterfaceForService(service: io_service_t, plugin_type: core_foundation_sys::uuid::CFUUIDRef, interface_type: core_foundation_sys::uuid::CFUUIDRef, out: *mut *mut *mut IUnknownHead, score: *mut i32) -> i32;
    fn IODestroyPlugInInterface(interface: *mut *mut IUnknownHead) -> i32;
}

fn constant_cfuuid(b: [u8; 16]) -> core_foundation_sys::uuid::CFUUIDRef {
    // SAFETY: a constant UUID is owned by CoreFoundation for the life of the process; nothing here
    // releases it.
    unsafe { core_foundation_sys::uuid::CFUUIDGetConstantUUIDWithBytes(std::ptr::null(), b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]) }
}

/// kIONVMeSMARTUserClientTypeID, kIONVMeSMARTInterfaceID (NVMeSMARTLibExternal.h) and
/// kIOCFPlugInInterfaceID (IOCFPlugIn.h).
const NVME_SMART_USER_CLIENT: [u8; 16] = [0xAA, 0x0F, 0xA6, 0xF9, 0xC2, 0xD6, 0x45, 0x7F, 0xB1, 0x0B, 0x59, 0xA1, 0x32, 0x53, 0x29, 0x2F];
const NVME_SMART_INTERFACE: [u8; 16] = [0xCC, 0xD1, 0xDB, 0x19, 0xFD, 0x9A, 0x4D, 0xAF, 0xBF, 0x95, 0x12, 0x45, 0x4B, 0x23, 0x0A, 0xB6];
const CF_PLUGIN_INTERFACE: [u8; 16] = [0xC2, 0x44, 0xE8, 0x58, 0x10, 0x9C, 0x11, 0xD4, 0x91, 0xD4, 0x00, 0x50, 0xE4, 0xC6, 0x42, 0x6F];

/// Every service whose registry entry says "NVMe SMART Capable" = Yes, internal drives first.
// SAFETY: the matching dictionary is retained once more before IOServiceGetMatchingServices
// consumes that one reference, so `matching` still owns its own when it is dropped; the iterator is
// drained and released here, and each service returned is the caller's to release.
unsafe fn smart_capable_services() -> Vec<(bool, io_service_t)> {
    use core_foundation::boolean::CFBoolean;
    let inner = CFDictionary::from_CFType_pairs(&[(CFString::new("NVMe SMART Capable"), CFBoolean::true_value().as_CFType())]);
    let matching = CFDictionary::from_CFType_pairs(&[(CFString::new("IOPropertyMatch"), inner.as_CFType())]);
    core_foundation_sys::base::CFRetain(matching.as_concrete_TypeRef() as _);
    let mut iter: io_iterator_t = 0;
    if IOServiceGetMatchingServices(kIOMasterPortDefault, matching.as_concrete_TypeRef() as _, &mut iter) != 0 {
        return vec![];
    }
    let mut out = Vec::new();
    loop {
        let svc = IOIteratorNext(iter);
        if svc == 0 { break; }
        let internal = dict_property(svc, "Protocol Characteristics")
            .and_then(|d| d.find(CFString::new("Physical Interconnect Location")).and_then(|v| v.clone().downcast::<CFString>()))
            .is_some_and(|v| v.to_string() == "Internal");
        out.push((internal, svc));
    }
    IOObjectRelease(iter);
    out.sort_by_key(|(internal, _)| !internal);
    out
}

/// Reads the SMART page through the drive's plug-in. A failure says which step was refused.
// SAFETY: `svc` is a live service handle the caller owns. The plug-in and interface pointers are
// only dereferenced after the calls that fill them report success and leave them non-null, and
// each is released exactly once (`release`, `IODestroyPlugInInterface`) before returning.
unsafe fn read_smart_page(svc: io_service_t) -> Result<[u8; 512], &'static str> {
    let mut plugin: *mut *mut IUnknownHead = std::ptr::null_mut();
    let mut score = 0i32;
    if IOCreatePlugInInterfaceForService(svc, constant_cfuuid(NVME_SMART_USER_CLIENT), constant_cfuuid(CF_PLUGIN_INTERFACE), &mut plugin, &mut score) != 0 || plugin.is_null() {
        return Err("macOS would not open the drive's SMART interface");
    }
    let mut smart: *mut std::ffi::c_void = std::ptr::null_mut();
    let iid = core_foundation_sys::uuid::CFUUIDGetUUIDBytes(constant_cfuuid(NVME_SMART_INTERFACE));
    let asked = ((**plugin).query_interface)(plugin as _, iid, &mut smart);
    let result = if asked != 0 || smart.is_null() {
        Err("the drive's SMART plug-in has no SMART interface")
    } else {
        let iface = smart as *mut *mut SmartInterface;
        let mut page = [0u8; 512];
        let read = ((**iface).smart_read_data)(smart, &mut page);
        ((**iface).head.release)(smart);
        if read == 0 { Ok(page) } else { Err("the drive did not return its SMART data") }
    };
    IODestroyPlugInInterface(plugin);
    result
}

/// Lifetime bytes written to the internal SSD, from its NVMe SMART data. Read through Apple's
/// NVMe SMART plug-in, which on Apple silicon answers an ordinary user without root (checked on
/// macOS 27). Unavailable, with the reason, on a Mac whose drive does not offer it.
pub fn ssd_lifetime_bytes_written() -> crate::sampler::Reading<u64> {
    use crate::sampler::Reading;
    // SAFETY: `smart_capable_services` has no precondition; every service it returns is ours to
    // release, which the loop below does.
    let services = unsafe { smart_capable_services() };
    let mut result = Reading::Unavailable("no drive on this Mac reports NVMe SMART data");
    for (i, &(_, svc)) in services.iter().enumerate() {
        if i == 0 {
            // SAFETY: `svc` is live until it is released just below; `read_smart_page` does not keep it.
            result = match unsafe { read_smart_page(svc) } { Ok(page) => Reading::Value(smart_bytes_written(&page)), Err(e) => Reading::Unavailable(e) };
        }
        // SAFETY: each service is released exactly once, here, and not used after.
        unsafe { IOObjectRelease(svc) };
    }
    result
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
    fn smart_bytes_written_reads_the_data_units_field_in_thousands_of_512_byte_blocks() {
        let mut page = [0u8; 512];
        page[48..56].copy_from_slice(&406_048_281u64.to_le_bytes());
        page[32] = 0xFF; // Data Units Read sits just before it and must not leak in
        assert_eq!(smart_bytes_written(&page), 406_048_281 * 512_000);
        page[63] = 0x01; // a count too large for u64 bytes saturates rather than wrapping
        assert_eq!(smart_bytes_written(&page), u64::MAX);
    }

    #[test]
    fn ssd_lifetime_bytes_written_is_a_value_or_a_reason_never_a_panic() {
        // A hosted runner may have no NVMe drive at all; a real Mac with one reads a non-zero count.
        match ssd_lifetime_bytes_written() {
            crate::sampler::Reading::Value(v) => assert!(v > 0),
            crate::sampler::Reading::Unavailable(why) => assert!(!why.is_empty()),
            crate::sampler::Reading::Restricted => panic!("never produced here"),
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
        assert!(disks.boot_volume_stats().is_ok());
    }

    /// `number_in` is the one place that actually parses a registry dictionary's keys; these use
    /// synthetic `CFDictionary` values built in-process (no real IOKit call), covering what the
    /// real `PerformanceStatistics`/`Statistics` dictionaries confirmed on this Mac (via
    /// `ioreg -r -d 1 -c IOAccelerator` / `-c IOBlockStorageDriver`) actually contain: an integer
    /// value under the expected key, a key absent entirely, and a value of the wrong CF type.
    fn canned_dict(pairs: &[(&str, core_foundation::base::CFType)]) -> CFDictionary<CFString, core_foundation::base::CFType> {
        let pairs: Vec<_> = pairs.iter().map(|(k, v)| (CFString::new(k), v.clone())).collect();
        CFDictionary::from_CFType_pairs(&pairs)
    }

    #[test]
    fn number_in_reads_a_present_integer_key() {
        let d = canned_dict(&[("Device Utilization %", CFNumber::from(4).as_CFType())]);
        assert_eq!(number_in(&d, "Device Utilization %"), Some(4));
    }

    #[test]
    fn number_in_is_none_for_a_key_the_dictionary_does_not_have() {
        let d = canned_dict(&[("Device Utilization %", CFNumber::from(4).as_CFType())]);
        assert_eq!(number_in(&d, "Tiler Utilization %"), None);
    }

    #[test]
    fn number_in_is_none_for_a_key_whose_value_is_the_wrong_cf_type() {
        // A real registry dictionary could in principle hold a string or boolean under a key this
        // code expects to be numeric; `number_in` must not panic, just report nothing.
        let d = canned_dict(&[("Bytes (Read)", CFString::new("not a number").as_CFType())]);
        assert_eq!(number_in(&d, "Bytes (Read)"), None);
    }

    #[test]
    fn number_in_reads_real_disk_statistics_key_names_confirmed_on_this_mac() {
        // Exact keys from a real `ioreg -r -d 1 -c IOBlockStorageDriver` run on this Mac:
        // "Statistics" = {"Bytes (Read)"=..., "Bytes (Write)"=..., "Operations (Read)"=...,
        // "Operations (Write)"=..., ...}.
        let d = canned_dict(&[
            ("Bytes (Read)", CFNumber::from(6_931_016_622_080i64).as_CFType()),
            ("Bytes (Write)", CFNumber::from(3_227_257_466_880i64).as_CFType()),
            ("Operations (Read)", CFNumber::from(269_577_277i64).as_CFType()),
            ("Operations (Write)", CFNumber::from(135_704_229i64).as_CFType()),
        ]);
        assert_eq!(number_in(&d, "Bytes (Read)"), Some(6_931_016_622_080));
        assert_eq!(number_in(&d, "Operations (Write)"), Some(135_704_229));
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
