// src/runtime/rng.rs
//! Random number utilities for PASTA runtime.
//!
//! This module attempts to use CPU hardware RNG instructions when available
//! (RDRAND / RDSEED on x86_64). If hardware RNG is not available or fails,
//! it falls back to a small, fast pseudo-random generator (xorshift64*).
//!
//! Design goals:
//! - Use hardware RNG when the CPU advertises it and the instruction succeeds.
//! - Keep a safe, ergonomic API: `Rng::next_u64()`, `next_u32()`, `fill_bytes()`.
//! - No external crates required; fallback PRNG is deterministic per-process
//!   but seeded from a combination of system time and address entropy.
//! - Guard assembly/intrinsics behind `cfg` checks so non-x86 targets compile.
//!
//! Notes:
//! - On x86_64 we prefer the `rdrand` instruction. If you want `rdseed` instead,
//!   you can call `try_rdseed_u64` (both are attempted where appropriate).
//! - The fallback PRNG is suitable for non-cryptographic uses (scheduling,
//!   randomized heuristics). If you need cryptographic RNG, prefer OS-provided
//!   sources (e.g., getrandom crate) or ensure `rdseed`/`rdrand` is available.

#![allow(dead_code)]

use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::atomic::{AtomicU64, Ordering};
use std::io::Read;

#[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
use core::arch::asm;

#[cfg(all(not(feature = "use_asm"), target_arch = "x86_64"))]
use core::arch::x86_64::{_rdrand64_step, _rdseed64_step};

/// Small wrapper RNG that prefers hardware RNG on supported platforms.
#[derive(Debug)]
pub struct Rng {
    /// If `true`, hardware RNG is available and will be attempted first.
    hw_available: bool,
    /// Fallback PRNG state (xorshift64*). Only used when hardware RNG is not available
    /// or when hardware instruction fails repeatedly.
    fallback_state: AtomicU64,
}

impl Rng {
    /// Create a new RNG instance. Seeds fallback PRNG from system time and pointer entropy.
    pub fn new() -> Self {
        let hw = Self::detect_hw();
        let seed = Self::seed_from_system();
        Self {
            hw_available: hw,
            fallback_state: AtomicU64::new(seed),
        }
    }

    /// Detect whether hardware RNG instructions are available on this target.
    ///
    /// On x86_64 this checks for `rdrand` support via the standard macro.
    /// For other architectures this returns false.
    fn detect_hw() -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            // is_x86_feature_detected! is a macro that expands to a runtime check.
            // It is safe to call on stable Rust.
            std::is_x86_feature_detected!("rdrand")
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }

    /// Seed fallback PRNG using system time and address entropy.
    fn seed_from_system() -> u64 {
        // Use system time nanoseconds as base
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u128)
            .unwrap_or(0u128);

        // Mix with address of a stack variable for some ASLR-derived entropy
        let stack_addr = &t as *const _ as usize as u128;

        // Combine and reduce to 64 bits
        let mut s = t ^ (stack_addr.rotate_left(13));
        // Avalanche a bit
        s = s.wrapping_mul(0x9E3779B97F4A7C15u128);
        (s as u64) ^ 0xA5A5_A5A5_A5A5_A5A5u64
    }

    /// Try to obtain a u64 from hardware RNG (RDRAND). Returns `Some(u64)` on success.
    ///
    /// On x86_64 we attempt either inline assembly (when `feature = "use_asm"`) or
    /// the compiler intrinsics. If the instruction fails, returns `None`.
    fn try_rdrand_u64(&self) -> Option<u64> {
        #[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
        {
            // Inline asm path: execute RDRAND and check CF flag.
            // Output in rax, CF in flags.
            let mut out: u64 = 0;
            let mut ok: u8;
            unsafe {
                asm!(
                    "rdrand {0}",
                    "setc {1}",
                    out(reg) out,
                    out(reg_byte) ok,
                    options(nomem, nostack, preserves_flags)
                );
            }
            if ok != 0 {
                Some(out)
            } else {
                None
            }
        }

        #[cfg(all(not(feature = "use_asm"), target_arch = "x86_64"))]
        {
            // Use the intrinsic which returns 1 on success and writes to the pointer.
                unsafe {
                    let mut val: u64 = 0;
                    let ok = _rdrand64_step(&mut val);
                    if ok == 1 {
                        Some(val)
                    } else {
                        None
                    }
                }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            None
        }
    }

    /// Try to obtain a u64 from RDSEED (stronger seed instruction). Returns `Some(u64)` on success.
    fn try_rdseed_u64(&self) -> Option<u64> {
        #[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
        {
            let mut out: u64 = 0;
            let mut ok: u8;
            unsafe {
                asm!(
                    "rdseed {0}",
                    "setc {1}",
                    out(reg) out,
                    out(reg_byte) ok,
                    options(nomem, nostack, preserves_flags)
                );
            }
            if ok != 0 {
                Some(out)
            } else {
                None
            }
        }

        #[cfg(all(not(feature = "use_asm"), target_arch = "x86_64"))]
        {
                unsafe {
                    let mut val: u64 = 0;
                    let ok = _rdseed64_step(&mut val);
                    if ok == 1 {
                        Some(val)
                    } else {
                        None
                    }
                }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            None
        }
    }

    /// Get the next random u64. Tries hardware RNG first (if available), then falls back.
    ///
    /// This function is safe to call from multiple threads; fallback_state uses an atomic.
    pub fn next_u64(&self) -> u64 {
        // First try OS-provided entropy (e.g., /dev/urandom) where available.
        // This is safer than directly executing CPU RNG instructions in
        // environments (emulators) that may misreport support and cause SIGILL.
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            let mut buf = [0u8; 8];
            if f.read_exact(&mut buf).is_ok() {
                return u64::from_le_bytes(buf);
            }
        }

        // If hardware advertised, try it a few times before falling back.
        if self.hw_available {
            // Prefer RDSEED if available (stronger), then RDRAND.
            if let Some(v) = self.try_rdseed_u64() {
                return v;
            }
            if let Some(v) = self.try_rdrand_u64() {
                return v;
            }
            // If hardware failed, fall through to fallback PRNG.
        }

        // Fallback: xorshift64* (simple, fast, non-cryptographic)
        self.xorshift64star()
    }

    /// Get next random u32 (lower 32 bits of next_u64).
    pub fn next_u32(&self) -> u32 {
        (self.next_u64() & 0xFFFF_FFFF) as u32
    }

    /// Fill the provided buffer with random bytes.
    pub fn fill_bytes(&self, out: &mut [u8]) {
        let mut i = 0usize;
        let len = out.len();
        while i < len {
            let v = self.next_u64();
            let bytes = v.to_le_bytes();
            let take = std::cmp::min(8, len - i);
            out[i..i + take].copy_from_slice(&bytes[..take]);
            i += take;
        }
    }

    /// Reseed the fallback PRNG with a new seed.
    pub fn reseed(&self, seed: u64) {
        self.fallback_state.store(seed, Ordering::SeqCst);
    }

    /// xorshift64* implementation using atomic state.
    ///
    /// This is intentionally simple and fast. Not suitable for crypto.
    fn xorshift64star(&self) -> u64 {
        // xorshift64* constants from Sebastiano Vigna
        // state must be non-zero
        let mut s = self.fallback_state.load(Ordering::Relaxed);
        if s == 0 {
            // reseed if zero
            s = Self::seed_from_system();
            self.fallback_state.store(s, Ordering::Relaxed);
        }

        // xorshift64*
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        let res = s.wrapping_mul(0x2545F4914F6CDD1Du64);

        // store updated state
        self.fallback_state.store(s, Ordering::Relaxed);
        res
    }
}

impl Default for Rng {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_generates_values() {
        let rng = Rng::new();
        let a = rng.next_u64();
        let b = rng.next_u64();
        // Very small chance of equality; just ensure function runs and returns values.
        assert_ne!(a, 0u64); // seed likely non-zero
        // We can't assert a != b deterministically, but we can at least call both.
        let _ = b;
    }

    #[test]
    fn fill_bytes_fills_buffer() {
        let rng = Rng::new();
        let mut buf = [0u8; 24];
        rng.fill_bytes(&mut buf);
        // Ensure not all zeros (very unlikely)
        assert!(buf.iter().any(|&b| b != 0));
    }

    #[test]
    fn reseed_changes_output() {
        let rng = Rng::new();
        let v1 = rng.next_u64();
        rng.reseed(0xDEADBEEFCAFEBABEu64);
        let v2 = rng.next_u64();
        // After reseed, output likely different
        assert_ne!(v1, v2);
    }

    #[test]
    fn hardware_path_does_not_panic_on_unsupported_arch() {
        // Constructing Rng on non-x86 should be fine
        let _ = Rng::new();
    }
}
