//! Small helpers for reading a request's query string and for secrets, shared by the server and the
//! removal route so neither depends on the other.

/// Decodes `%XX` and `+`. A `%` not followed by two hex digits is kept as a literal `%`, never
/// guessed at, and nothing here can panic on multi-byte text.
pub fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let (mut out, mut i) = (Vec::with_capacity(b.len()), 0);
    while i < b.len() {
        match b[i] {
            b'%' => match b.get(i + 1..i + 3).filter(|h| h.iter().all(u8::is_ascii_hexdigit)) {
                Some(h) => { out.push(u8::from_str_radix(std::str::from_utf8(h).unwrap_or("00"), 16).unwrap_or(b'?')); i += 3; }
                None => { out.push(b'%'); i += 1; }
            },
            b'+' => { out.push(b' '); i += 1 }
            c => { out.push(c); i += 1 }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 128 random bits as hex, for the per-run token and the removal confirmations.
pub fn random_token() -> std::io::Result<String> {
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut b))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Compares two secrets without stopping at the first difference.
pub fn secrets_equal(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) { diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0)); }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_never_panics_and_only_accepts_hex_escapes() {
        assert_eq!(urldecode("a%20b+c%2Fd"), "a b c/d");
        assert_eq!(urldecode("%€"), "%€", "a percent sign before multi-byte text is literal");
        assert_eq!(urldecode("%+1"), "% 1", "a plus is not a hex digit");
        assert_eq!(urldecode("100%"), "100%");
        assert_eq!(urldecode("%4"), "%4");
        assert_eq!(urldecode("%zz%4g"), "%zz%4g");
        assert_eq!(urldecode("é%C3%A9"), "éé");
        for s in ["%", "%%", "%é", "é%", "%€€", "%a€", "%\u{1F600}1"] { let _ = urldecode(s); }
    }

    #[test]
    fn secrets_compare_by_value_and_length() {
        assert!(secrets_equal("abc", "abc") && secrets_equal("", ""));
        for (a, b) in [("abc", "abd"), ("abc", "ab"), ("ab", "abc"), ("", "a")] { assert!(!secrets_equal(a, b), "{a} {b}"); }
        assert_eq!(random_token().unwrap().len(), 32);
    }
}
