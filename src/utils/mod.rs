// src/utils/mod.rs
//! Utility helpers for PASTA
//!
//! This module collects small, broadly useful utilities used across the project:
//! - `errors` — centralized error type and helpers
//! - `helpers` — small helper functions (identifiers, parsing, hex, time, etc.)
//! - `logging` — lightweight, dependency-free logging facility
//!
//! Each submodule is intentionally small and well-tested. Re-export commonly
//! used types and functions here for convenient access.

pub mod errors;
pub mod helpers;
pub mod logging;

pub use errors::{Error, ErrorKind, Result as ErrorResult};
pub use helpers::{
    approx_eq, bytes_to_hex, clamp, duration_to_human, hex_to_bytes, hex_val, is_valid_ident,
    join_with_limit, now_millis, parse_f64, safe_div, to_snake_case,
};
pub use logging::{init_logger, log, set_level, set_level_from_str, LogLevel};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::errors::ErrorKind;
    use crate::utils::helpers::{is_valid_ident, to_snake_case, bytes_to_hex, hex_to_bytes};
    use crate::utils::logging::{init_logger, LogLevel};
    use std::fs;

    #[test]
    fn helpers_basic() {
        assert!(is_valid_ident("abc_123"));
        assert!(!is_valid_ident("1abc"));
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
        let data = b"\x01\x02\xff";
        let hx = bytes_to_hex(data);
        assert_eq!(hex_to_bytes(&hx).unwrap(), data);
    }

    #[test]
    fn errors_and_result_alias() {
        let e = crate::utils::errors::Error::new(ErrorKind::InvalidInput, "bad");
        let s = format!("{}", e);
        assert!(s.contains("InvalidInput") || s.contains("bad"));
    }

    #[test]
    fn logging_init_and_write() {
        // initialize a temporary log file
        let mut tmp = std::env::temp_dir();
        tmp.push("pasta_utils_logging_test.log");
        let _ = fs::remove_file(&tmp);

        init_logger(LogLevel::Debug, Some(tmp.to_str().unwrap()), false).unwrap();
        pasta_info!("utils test info {}", 1);
        pasta_debug!("utils test debug {}", 2);

        // allow flush
        std::thread::sleep(std::time::Duration::from_millis(10));
        let contents = fs::read_to_string(&tmp).expect("log file should exist");
        assert!(contents.contains("utils test info") || contents.contains("utils test debug"));

        let _ = fs::remove_file(&tmp);
    }
}
