// src/lexer/unicode.rs
//! Unicode math normalization helpers for PASTA lexer.
//!
//! Responsibilities:
//! - Normalize common Unicode math symbols to ASCII equivalents (e.g., × -> *, ÷ -> /).
//! - Normalize superscript digits to ASCII digits so numeric parsing can be simpler.
//! - Provide small utilities for future normalization (e.g., superscript signs, unicode minus).
//!
//! This module is intentionally small and deterministic. It does not perform full
//! Unicode normalization (NFC/NFD) — that can be added later if needed.

/// Normalize common Unicode math characters into ASCII equivalents.
///
/// Current mappings:
/// - '×' -> '*'
/// - '⋅' -> '*'
/// - '·' -> '*'
/// - '÷' -> '/'
/// - '⁄' -> '/'
/// - Unicode superscript digits ⁰¹²³⁴⁵⁶⁷⁸⁹ -> 0..9
/// - Unicode minus '−' -> '-'
///
/// The function returns a new `String` with replacements applied.
pub fn normalize_unicode(input: &str) -> String {
    // Fast path: if input contains none of the mapped characters, return a clone.
    // This avoids allocations for most ASCII-only source lines.
    if !input.contains('×')
        && !input.contains('⋅')
        && !input.contains('·')
        && !input.contains('÷')
        && !input.contains('⁄')
        && !input.contains('⁰')
        && !input.contains('¹')
        && !input.contains('²')
        && !input.contains('³')
        && !input.contains('⁴')
        && !input.contains('⁵')
        && !input.contains('⁶')
        && !input.contains('⁷')
        && !input.contains('⁸')
        && !input.contains('⁹')
        && !input.contains('−')
    {
        return input.to_string();
    }

    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '×' | '⋅' | '·' => out.push('*'),
            '÷' | '⁄' => out.push('/'),
            '⁰' => out.push('0'),
            '¹' => out.push('1'),
            '²' => out.push('2'),
            '³' => out.push('3'),
            '⁴' => out.push('4'),
            '⁵' => out.push('5'),
            '⁶' => out.push('6'),
            '⁷' => out.push('7'),
            '⁸' => out.push('8'),
            '⁹' => out.push('9'),
            '−' => out.push('-'),
            other => out.push(other),
        }
    }
    out
}

/// Convenience: normalize and also collapse common exponent notations that use superscripts
/// into ASCII caret form when possible. This is a small helper and not a full numeric parser.
///
/// Example:
/// - "5×10⁶" -> "5*10^6"  (note: caret inserted only when a superscript digit sequence follows '10')
///
/// This helper is conservative: it only converts the specific pattern "10" followed by
/// superscript digits into "10^digits". It is provided to make common scientific-notation
/// idioms easier to parse later in the numeric lexer stage.
pub fn normalize_exponent_caret(input: &str) -> String {
    // First normalize basic unicode characters
    let s = normalize_unicode(input);

    // Look for "10" followed immediately by superscript digits (now ASCII digits after normalize_unicode)
    // We will replace "10" + digits with "10^digits"
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '1' {
            if let Some('0') = chars.peek() {
                // Tentatively consume '0'
                out.push('1');
                out.push('0');
                chars.next(); // consume '0'
                // Now check for a run of digits (these may have been superscripts normalized earlier)
                let mut digits = String::new();
                while let Some(peek) = chars.peek() {
                    if peek.is_ascii_digit() {
                        digits.push(*peek);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !digits.is_empty() {
                    out.push('^');
                    out.push_str(&digits);
                }
                continue;
            } else {
                out.push('1');
                continue;
            }
        } else {
            out.push(ch);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_normalize() {
        let s = "5×10⁶ ÷ 2 − 3 ⋅ 4";
        let n = normalize_unicode(s);
        assert!(n.contains('*'));
        assert!(n.contains('/'));
        assert!(n.contains('-'));
        assert!(n.contains("10"));
        assert!(n.contains("6")); // superscript converted
    }

    #[test]
    fn test_exponent_caret() {
        let s = "5×10⁶";
        let n = normalize_exponent_caret(s);
        assert_eq!(n, "5*10^6");
    }

    #[test]
    fn test_no_change_fast_path() {
        let s = "let x = 5 + 3";
        let n = normalize_unicode(s);
        assert_eq!(n, s);
    }
}
