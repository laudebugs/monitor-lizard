//! How many network interfaces are up and running. The byte counters alone cannot say: an
//! interface that is down and one that is idle both give a rate of zero, and "no connection" must
//! not be drawn as an idle zero.
use std::ffi::CStr;

/// One address entry from `getifaddrs`, reduced to what the count needs. `routable` is about the
/// address itself: link-local addresses (169.254/16, fe80::/10) exist on tunnel and peer-to-peer
/// interfaces that macOS keeps up with no network behind them.
pub struct IfEntry { pub name: String, pub loopback: bool, pub up: bool, pub running: bool, pub routable: bool }

/// An IPv4 address that can reach beyond the local link.
pub fn routable_v4(a: [u8; 4]) -> bool { a != [0; 4] && a[0] != 127 && !(a[0] == 169 && a[1] == 254) }

/// An IPv6 address that can reach beyond the local link.
pub fn routable_v6(a: [u8; 16]) -> bool { a != [0; 16] && a != { let mut l = [0u8; 16]; l[15] = 1; l } && !(a[0] == 0xfe && a[1] & 0xc0 == 0x80) }

/// Distinct non-loopback interfaces that are up, running and hold a routable address.
pub fn count_active(entries: &[IfEntry]) -> u32 {
    let mut names: Vec<&str> = entries.iter().filter(|e| !e.loopback && e.up && e.running && e.routable).map(|e| e.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    names.len() as u32
}

/// Every address entry on the machine, or `None` when the list cannot be read.
pub fn read_entries() -> Option<Vec<IfEntry>> {
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills `head` with a list that freeifaddrs releases below; a failure leaves it null.
    if unsafe { libc::getifaddrs(&mut head) } != 0 { return None; }
    let mut entries = Vec::new();
    let mut cur = head;
    while !cur.is_null() {
        // SAFETY: `cur` is a node of the list above, valid until freeifaddrs; ifa_name is NUL-terminated.
        let ifa = unsafe { &*cur };
        let name = unsafe { CStr::from_ptr(ifa.ifa_name) }.to_string_lossy().into_owned();
        let flags = ifa.ifa_flags as i32;
        let routable = if ifa.ifa_addr.is_null() { false } else {
            // SAFETY: a non-null ifa_addr points at a sockaddr whose sa_family says which larger struct it is,
            // and getifaddrs allocates the full struct for that family.
            match i32::from(unsafe { (*ifa.ifa_addr).sa_family }) {
                libc::AF_INET => routable_v4(unsafe { (*(ifa.ifa_addr as *const libc::sockaddr_in)).sin_addr.s_addr }.to_ne_bytes()),
                libc::AF_INET6 => routable_v6(unsafe { (*(ifa.ifa_addr as *const libc::sockaddr_in6)).sin6_addr.s6_addr }),
                _ => false,
            }
        };
        entries.push(IfEntry { name, loopback: flags & libc::IFF_LOOPBACK != 0, up: flags & libc::IFF_UP != 0, running: flags & libc::IFF_RUNNING != 0, routable });
        cur = ifa.ifa_next;
    }
    // SAFETY: `head` came from a successful getifaddrs and is freed once.
    unsafe { libc::freeifaddrs(head) };
    Some(entries)
}

/// `None` when the list cannot be read, so the page says "unavailable" rather than "no connection".
pub fn up_count() -> Option<u32> { read_entries().map(|e| count_active(&e)) }

#[cfg(test)]
mod tests {
    use super::*;
    fn e(name: &str, loopback: bool, up: bool, running: bool, routable: bool) -> IfEntry { IfEntry { name: name.into(), loopback, up, running, routable } }

    #[test]
    fn a_routable_interface_counts_once_even_with_several_addresses() {
        let list = [e("en0", false, true, true, true), e("en0", false, true, true, true), e("en0", false, true, true, false)];
        assert_eq!(count_active(&list), 1);
    }

    #[test]
    fn a_vpn_tunnel_with_a_routable_address_is_a_connection() {
        assert_eq!(count_active(&[e("utun4", false, true, true, true)]), 1);
    }

    #[test]
    fn loopback_is_excluded_by_flag_not_by_name() {
        assert_eq!(count_active(&[e("lo0", true, true, true, true), e("lo1", true, true, true, true)]), 0);
        assert_eq!(count_active(&[e("lo0", false, true, true, true)]), 1, "a name alone proves nothing");
    }

    #[test]
    fn interfaces_with_only_link_local_addresses_are_not_a_connection() {
        // utun, awdl, llw and anpi keep a fe80:: address and stay up with no network behind them.
        let list = [e("utun0", false, true, true, routable_v6(addr6("fe80::1"))), e("awdl0", false, true, true, routable_v6(addr6("fe80::2"))),
            e("en0", false, true, true, routable_v4([169, 254, 12, 3]))];
        assert_eq!(count_active(&list), 0);
    }

    #[test]
    fn a_down_or_stopped_interface_is_not_a_connection() {
        let list = [e("en0", false, false, false, true), e("en1", false, true, false, true)];
        assert_eq!(count_active(&list), 0);
    }

    fn addr6(s: &str) -> [u8; 16] { s.parse::<std::net::Ipv6Addr>().unwrap().octets() }

    #[test]
    fn address_ranges_are_classified() {
        assert!(routable_v4([192, 168, 1, 20]) && routable_v4([10, 8, 0, 2]) && routable_v4([169, 255, 0, 1]));
        assert!(!routable_v4([169, 254, 0, 5]) && !routable_v4([0; 4]) && !routable_v4([127, 0, 0, 1]));
        assert!(routable_v6(addr6("2001:db8::7")) && routable_v6(addr6("fd00::1")));
        assert!(!routable_v6(addr6("fe80::1")) && !routable_v6(addr6("febf::1")) && !routable_v6(addr6("::1")) && !routable_v6(addr6("::")));
        assert!(routable_v6(addr6("fec0::1")), "fec0 is outside fe80::/10");
    }

    #[test]
    fn the_real_list_can_be_read() { assert!(up_count().is_some()); }
}
