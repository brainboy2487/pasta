// src/utils/helpers.rs
//! Small helper utilities used across the PASTA codebase.
//!
//! This module collects a handful of small, well-tested helper functions that
//! don't belong to a single subsystem. Keep helpers small and dependency-free.
//!
//! Provided utilities:
//! - Identifier helpers: `is_valid_ident`, `to_snake_case`
//! - Numeric parsing/formatting: `parse_f64`, `safe_div`, `approx_eq`
//! - Byte/hex helpers: `bytes_to_hex`, `hex_to_bytes`
//! - Time helpers: `now_millis`, `duration_to_human`
//! - Misc: `clamp`, `join_with_limit`
//!
//! These are intentionally generic and small so they can be reused in tests and
//! multiple modules without creating coupling.

use std::time::{SystemTime, UNIX_EPOCH};
use std::num::ParseIntError;
use crate::utils::errors::{Error, ErrorKind, Result};

/// Validate a simple identifier (letters, digits, underscore; must not start with digit).
///
/// This mirrors the lightweight identifier rules used by the parser and ASM runtime.
pub fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    for ch in chars {
        if !(ch.is_ascii_alphanumeric() || ch == '_') {
            return false;
        }
    }
    true
}

/// Convert a string to snake_case in a conservative way.
///
/// This is not a full Unicode-aware conversion; it is intended for ASCII identifiers.
pub fn to_snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_lower = false;
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            if prev_lower {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower = false;
        } else if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            // replace other characters with underscore (collapse consecutive)
            if !out.ends_with('_') {
                out.push('_');
            }
            prev_lower = false;
        }
    }
    // Trim leading/trailing underscores
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        // fallback to original lowercased
        s.to_ascii_lowercase()
    } else {
        trimmed
    }
}

/// Parse a floating point number from a string, returning a helpful error on failure.
pub fn parse_f64(s: &str) -> Result<f64> {
    s.trim().parse::<f64>().map_err(|e| {
        Error::with_source(
            ErrorKind::InvalidInput,
            format!("failed to parse '{}' as number", s),
            e,
        )
    })
}

/// Safe division that returns an error on division by zero.
pub fn safe_div(a: f64, b: f64) -> Result<f64> {
    if b == 0.0 {
        Err(Error::new(ErrorKind::InvalidInput, "division by zero"))
    } else {
        Ok(a / b)
    }
}

/// Approximate equality for floating point values with a relative epsilon.
pub fn approx_eq(a: f64, b: f64, rel_eps: f64) -> bool {
    if a == b {
        return true;
    }
    let diff = (a - b).abs();
    let largest = a.abs().max(b.abs()).max(1.0);
    diff <= largest * rel_eps
}

/// Convert bytes to a lowercase hex string.
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Convert a hex string (with optional 0x prefix) to bytes.
///
/// Returns an error if the string contains invalid hex or has odd length.
pub fn hex_to_bytes(s: &str) -> Result<Vec<u8>> {
    let mut src = s.trim();
    if src.starts_with("0x") || src.starts_with("0X") {
        src = &src[2..];
    }
    if src.len() % 2 != 0 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "hex string must have even length",
        ));
    }
    let mut out = Vec::with_capacity(src.len() / 2);
    let mut chars = src.as_bytes();
    let mut i = 0usize;
    while i < chars.len() {
        let hi = hex_val(chars[i])?;
        let lo = hex_val(chars[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("invalid hex digit '{}'", b as char),
        )),
    }
}

/// Return current system time in milliseconds since UNIX epoch.
///
/// If system time cannot be obtained, returns 0.
pub fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Convert a duration in milliseconds to a human-friendly string.
///
/// Examples:
/// - 123 -> "123ms"
/// - 1500 -> "1.5s"
/// - 90_000 -> "1m30s"
pub fn duration_to_human(ms: u128) -> String {
    if ms < 1000 {
        return format!("{}ms", ms);
    }
    let secs = ms / 1000;
    if secs < 60 {
        // show one decimal place for sub-second precision when appropriate
        let rem_ms = ms % 1000;
        if rem_ms == 0 {
            return format!("{}s", secs);
        } else {
            let frac = (rem_ms as f64) / 1000.0;
            return format!("{:.1}s", (secs as f64) + frac);
        }
    }
    let mins = secs / 60;
    let rem_secs = secs % 60;
    if rem_secs == 0 {
        format!("{}m", mins)
    } else {
        format!("{}m{}s", mins, rem_secs)
    }
}

/// Clamp a value between min and max.
pub fn clamp<T: PartialOrd>(v: T, min: T, max: T) -> T {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// Join an iterator of strings but limit the total output length (approximate).
///
/// If the joined result would exceed `max_len`, the result is truncated and
/// an ellipsis (`"..."`) is appended.
pub fn join_with_limit<I>(iter: I, sep: &str, max_len: usize) -> String
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let mut out = String::new();
    let mut first = true;
    for item in iter {
        if !first {
            out.push_str(sep);
        }
        first = false;
        let s = item.as_ref();
        if out.len() + s.len() > max_len {
            // append as much as we can and break
            let remaining = if max_len > out.len() + 3 {
                max_len - out.len() - 3
            } else {
                0
            };
            if remaining > 0 {
                out.push_str(&s[..remaining]);
            }
            out.push_str("...");
            break;
        } else {
            out.push_str(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ident_valid_and_invalid() {
        assert!(is_valid_ident("abc"));
        assert!(is_valid_ident("_x1"));
        assert!(!is_valid_ident("1abc"));
        assert!(!is_valid_ident("a-b"));
    }

    #[test]
    fn snake_case_basic() {
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
        assert_eq!(to_snake_case("HTTPServer"), "http_server");
        assert_eq!(to_snake_case("a b"), "a_b");
    }

    #[test]
    fn parse_f64_ok_and_err() {
        assert_eq!(parse_f64("3.14").unwrap(), 3.14);
        assert!(parse_f64("notnum").is_err());
    }

    #[test]
    fn safe_div_behaviour() {
        assert_eq!(safe_div(6.0, 3.0).unwrap(), 2.0);
        assert!(safe_div(1.0, 0.0).is_err());
    }

    #[test]
    fn approx_eq_tests() {
        assert!(approx_eq(1.0, 1.0 + 1e-12, 1e-9));
        assert!(!approx_eq(1.0, 1.1, 1e-6));
    }

    #[test]
    fn hex_roundtrip() {
        let data = b"\x00\x01\xab\xff";
        let hx = bytes_to_hex(data);
        assert_eq!(hx, "0001abff");
        let back = hex_to_bytes(&hx).unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn hex_invalid() {
        assert!(hex_to_bytes("abc").is_err());
        assert!(hex_to_bytes("zz").is_err());
    }

    #[test]
    fn now_and_duration() {
        let ms = now_millis();
        assert!(ms > 0);
        assert_eq!(duration_to_human(123), "123ms");
        assert_eq!(duration_to_human(1500), "1.5s");
        assert_eq!(duration_to_human(90_000), "1m30s");
    }

    #[test]
    fn clamp_and_join() {
        assert_eq!(clamp(5, 1, 10), 5);
        assert_eq!(clamp(0, 1, 10), 1);
        let v = vec!["one", "two", "three"];
        assert_eq!(join_with_limit(v.iter(), ",", 100), "one,two,three");
        let v2 = vec!["longword", "anotherlongword"];
        let j = join_with_limit(v2.iter(), ",", 8);
        assert!(j.ends_with("..."));
    }
}
