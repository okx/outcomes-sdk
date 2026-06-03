//! Lowercase hex encoding/decoding helpers shared across the signing module.
//!
//! Single source of truth for nibble-level conversions used by both the ECDSA
//! signing pipeline (multi-byte buffers) and the client_order_id module (single-nibble
//! region/env prefixes).

/// Encode a low nibble (0..=15) as a lowercase hex char.
pub(crate) fn hex_char(n: u8) -> char {
    let n = n & 0x0f;
    if n < 10 {
        (b'0' + n) as char
    } else {
        (b'a' + (n - 10)) as char
    }
}

/// Parse a single ASCII hex byte into its nibble value (0..=15).
pub(crate) fn parse_hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Encode a byte slice as a lowercase hex string (no `0x` prefix).
pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(hex_char(b >> 4));
        s.push(hex_char(b & 0x0f));
    }
    s
}

/// Decode a lowercase- or uppercase-hex string into bytes. Errors on
/// odd-length input or any non-hex character.
pub(crate) fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("odd-length hex string".to_string());
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let hi = parse_hex_nibble(bytes[i]).ok_or_else(|| format!("invalid hex at pos {i}"))?;
        let lo = parse_hex_nibble(bytes[i + 1])
            .ok_or_else(|| format!("invalid hex at pos {}", i + 1))?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_char_round_trip() {
        for n in 0u8..16 {
            let c = hex_char(n);
            let parsed = parse_hex_nibble(c as u8).unwrap_or_else(|| unreachable!());
            assert_eq!(parsed, n);
        }
    }

    #[test]
    fn hex_encode_round_trip() {
        let data = [0xde, 0xad, 0xbe, 0xef];
        let encoded = hex_encode(&data);
        assert_eq!(encoded, "deadbeef");
        let decoded = hex_decode(&encoded).unwrap_or_else(|_| unreachable!());
        assert_eq!(decoded, data);
    }

    #[test]
    fn hex_decode_accepts_uppercase() {
        let decoded = hex_decode("FF00").unwrap_or_else(|_| unreachable!());
        assert_eq!(decoded, vec![0xff, 0x00]);
    }

    #[test]
    fn hex_decode_rejects_odd_length() {
        assert!(hex_decode("abc").is_err());
    }

    #[test]
    fn hex_decode_rejects_non_hex() {
        assert!(hex_decode("zzzz").is_err());
    }
}
