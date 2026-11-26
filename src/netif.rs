//! How many network interfaces are up and running. The byte counters alone cannot say: an
//! interface that is down and one that is idle both give a rate of zero, and "no connection" must
//! not be drawn as an idle zero.
use std::ffi::CStr;

/// One address entry from `getifaddrs`, reduced to what the count needs.
pub struct IfEntry { pub name: String, pub up: bool, pub running: bool, pub has_ip: bool }

/// Distinct interfaces that are up, running and carry an IP address, leaving out loopback.
pub fn count_active(entries: &[IfEntry]) -> u32 {
    let mut names: Vec<&str> = entries.iter().filter(|e| e.up && e.running && e.has_ip && e.name != "lo0").map(|e| e.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    names.len() as u32
}

/// `None` when the list cannot be read, so the page says "unavailable" rather than "no connection".
pub fn up_count() -> Option<u32> {
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills `head` with a list that freeifaddrs releases below; a failure leaves it null.
    if unsafe { libc::getifaddrs(&mut head) } != 0 { return None; }
    let mut entries = Vec::new();
    let mut cur = head;
    while !cur.is_null() {
        // SAFETY: `cur` is a node of the list above, valid until freeifaddrs; ifa_name is NUL-terminated.
        let ifa = unsafe { &*cur };
        let name = unsafe { CStr::from_ptr(ifa.ifa_name) }.to_string_lossy().into_owned();
        let family = if ifa.ifa_addr.is_null() { -1 } else { i32::from(unsafe { (*ifa.ifa_addr).sa_family }) };
        let flags = ifa.ifa_flags as i32;
        entries.push(IfEntry { name, up: flags & libc::IFF_UP != 0, running: flags & libc::IFF_RUNNING != 0, has_ip: family == libc::AF_INET || family == libc::AF_INET6 });
        cur = ifa.ifa_next;
    }
    // SAFETY: `head` came from a successful getifaddrs and is freed once.
    unsafe { libc::freeifaddrs(head) };
    Some(count_active(&entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn e(name: &str, up: bool, running: bool, has_ip: bool) -> IfEntry { IfEntry { name: name.into(), up, running, has_ip } }

    #[test]
    fn counts_each_working_interface_once_and_skips_loopback() {
        let list = [e("lo0", true, true, true), e("en0", true, true, true), e("en0", true, true, true), e("utun0", true, true, true)];
        assert_eq!(count_active(&list), 2);
    }

    #[test]
    fn a_down_or_addressless_interface_is_not_a_connection() {
        let list = [e("en0", false, false, true), e("en1", true, true, false), e("awdl0", true, false, true)];
        assert_eq!(count_active(&list), 0);
    }

    #[test]
    fn the_real_list_can_be_read() { assert!(up_count().is_some()); }
}
