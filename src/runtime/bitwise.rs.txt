// src/runtime/bitwise.rs
//! Bitwise operations for PASTA runtime.
//!
//! This module provides a small, well-documented set of bitwise primitives.
//! By default the implementations use idiomatic Rust operations which the
//! optimizer lowers to single CPU instructions. When the optional Cargo feature
//! `use_asm` is enabled on `x86_64` targets, platform-specific inline assembly
//! is used for a handful of operations (rotates and shifts) to demonstrate a
//! low-level path. The assembly path is guarded and optional — the pure Rust
//! path is portable, safe, and equally fast on modern compilers.
//!
//! Provided functions:
//! - `and_u64`, `or_u64`, `xor_u64`, `not_u64`
//! - `shl_u64`, `shr_u64` (logical shifts)
//! - `rol_u64`, `ror_u64` (rotates)
//! - `and_u32`, `or_u32`, `xor_u32`, `not_u32`, `shl_u32`, `shr_u32`, `rol_u32`, `ror_u32`
//!
//! Safety:
//! - The assembly implementations use `unsafe` inline `asm!` but are small and
//!   fully encapsulated. They are only compiled when `feature = "use_asm"` and
//!   `target_arch = "x86_64"`. If you need other architectures, add guarded
//!   implementations similarly.
//!
//! Note:
//! - On stable Rust and modern compilers, plain Rust bitwise ops compile to
//!   single instructions and are typically the best choice. The assembly path
//!   exists only for explicit low-level control when requested.

#[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
use core::arch::asm;

/// Bitwise AND for u64.
#[inline]
pub fn and_u64(a: u64, b: u64) -> u64 {
    // Pure Rust is optimal; compiler emits `and` instruction.
    a & b
}

/// Bitwise OR for u64.
#[inline]
pub fn or_u64(a: u64, b: u64) -> u64 {
    a | b
}

/// Bitwise XOR for u64.
#[inline]
pub fn xor_u64(a: u64, b: u64) -> u64 {
    a ^ b
}

/// Bitwise NOT for u64.
#[inline]
pub fn not_u64(a: u64) -> u64 {
    !a
}

/// Logical left shift for u64 by `shift` bits (shift masked to 0..63).
#[inline]
pub fn shl_u64(a: u64, shift: u32) -> u64 {
    // Mask shift to 0..63 to match CPU behavior for x86_64.
    let s = (shift & 0x3F) as u32;
    #[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
    {
        // Use `sal` instruction via inline asm for demonstration.
        let mut out = a;
        unsafe {
            asm!(
                "sal {1:e}, {0}",
                inout(reg) out,
                in(reg) s,
                options(pure, nomem, nostack)
            );
        }
        out
    }
    #[cfg(not(all(feature = "use_asm", target_arch = "x86_64")))]
    {
        a.wrapping_shl(s)
    }
}

/// Logical right shift for u64 by `shift` bits (zero-fill).
#[inline]
pub fn shr_u64(a: u64, shift: u32) -> u64 {
    let s = (shift & 0x3F) as u32;
    #[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
    {
        let mut out = a;
        unsafe {
            asm!(
                "shr {1:e}, {0}",
                inout(reg) out,
                in(reg) s,
                options(pure, nomem, nostack)
            );
        }
        out
    }
    #[cfg(not(all(feature = "use_asm", target_arch = "x86_64")))]
    {
        a.wrapping_shr(s)
    }
}

/// Rotate left for u64 by `rot` bits.
#[inline]
pub fn rol_u64(a: u64, rot: u32) -> u64 {
    let r = (rot & 0x3F) as u32;
    #[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
    {
        let mut out = a;
        unsafe {
            asm!(
                "rol {1:e}, {0}",
                inout(reg) out,
                in(reg) r,
                options(pure, nomem, nostack)
            );
        }
        out
    }
    #[cfg(not(all(feature = "use_asm", target_arch = "x86_64")))]
    {
        a.rotate_left(r)
    }
}

/// Rotate right for u64 by `rot` bits.
#[inline]
pub fn ror_u64(a: u64, rot: u32) -> u64 {
    let r = (rot & 0x3F) as u32;
    #[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
    {
        let mut out = a;
        unsafe {
            asm!(
                "ror {1:e}, {0}",
                inout(reg) out,
                in(reg) r,
                options(pure, nomem, nostack)
            );
        }
        out
    }
    #[cfg(not(all(feature = "use_asm", target_arch = "x86_64")))]
    {
        a.rotate_right(r)
    }
}

// ---------------------------
// 32-bit variants
// ---------------------------

#[inline]
pub fn and_u32(a: u32, b: u32) -> u32 {
    a & b
}

#[inline]
pub fn or_u32(a: u32, b: u32) -> u32 {
    a | b
}

#[inline]
pub fn xor_u32(a: u32, b: u32) -> u32 {
    a ^ b
}

#[inline]
pub fn not_u32(a: u32) -> u32 {
    !a
}

#[inline]
pub fn shl_u32(a: u32, shift: u32) -> u32 {
    let s = (shift & 0x1F) as u32;
    #[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
    {
        let mut out = a as u32;
        unsafe {
            asm!(
                "sal {1:e}, {0}",
                inout(reg) out,
                in(reg) s,
                options(pure, nomem, nostack)
            );
        }
        out
    }
    #[cfg(not(all(feature = "use_asm", target_arch = "x86_64")))]
    {
        a.wrapping_shl(s)
    }
}

#[inline]
pub fn shr_u32(a: u32, shift: u32) -> u32 {
    let s = (shift & 0x1F) as u32;
    #[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
    {
        let mut out = a as u32;
        unsafe {
            asm!(
                "shr {1:e}, {0}",
                inout(reg) out,
                in(reg) s,
                options(pure, nomem, nostack)
            );
        }
        out
    }
    #[cfg(not(all(feature = "use_asm", target_arch = "x86_64")))]
    {
        a.wrapping_shr(s)
    }
}

#[inline]
pub fn rol_u32(a: u32, rot: u32) -> u32 {
    let r = (rot & 0x1F) as u32;
    #[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
    {
        let mut out = a as u32;
        unsafe {
            asm!(
                "rol {1:e}, {0}",
                inout(reg) out,
                in(reg) r,
                options(pure, nomem, nostack)
            );
        }
        out
    }
    #[cfg(not(all(feature = "use_asm", target_arch = "x86_64")))]
    {
        a.rotate_left(r)
    }
}

#[inline]
pub fn ror_u32(a: u32, rot: u32) -> u32 {
    let r = (rot & 0x1F) as u32;
    #[cfg(all(feature = "use_asm", target_arch = "x86_64"))]
    {
        let mut out = a as u32;
        unsafe {
            asm!(
                "ror {1:e}, {0}",
                inout(reg) out,
                in(reg) r,
                options(pure, nomem, nostack)
            );
        }
        out
    }
    #[cfg(not(all(feature = "use_asm", target_arch = "x86_64")))]
    {
        a.rotate_right(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_u64_ops() {
        assert_eq!(and_u64(0b1100, 0b1010), 0b1000);
        assert_eq!(or_u64(0b1100, 0b0011), 0b1111);
        assert_eq!(xor_u64(0b1100, 0b1010), 0b0110);
        assert_eq!(not_u64(0b0u64), !0u64);
    }

    #[test]
    fn test_shifts_and_rotates_u64() {
        let v = 0x0123_4567_89AB_CDEFu64;
        assert_eq!(shl_u64(v, 8), v.wrapping_shl(8));
        assert_eq!(shr_u64(v, 8), v.wrapping_shr(8));
        assert_eq!(rol_u64(0x8000_0000_0000_0001u64, 1), 0x0000_0000_0000_0003u64);
        assert_eq!(ror_u64(0x0000_0000_0000_0003u64, 1), 0x8000_0000_0000_0001u64);
    }

    #[test]
    fn test_basic_u32_ops() {
        assert_eq!(and_u32(0b1100, 0b1010), 0b1000);
        assert_eq!(or_u32(0b1100, 0b0011), 0b1111);
        assert_eq!(xor_u32(0b1100, 0b1010), 0b0110);
        assert_eq!(not_u32(0b0u32), !0u32);
    }

    #[test]
    fn test_shifts_and_rotates_u32() {
        let v = 0x89AB_CDEFu32;
        assert_eq!(shl_u32(v, 8), v.wrapping_shl(8));
        assert_eq!(shr_u32(v, 8), v.wrapping_shr(8));
        assert_eq!(rol_u32(0x8000_0001u32, 1), 0x0000_0003u32);
        assert_eq!(ror_u32(0x0000_0003u32, 1), 0x8000_0001u32);
    }

    #[test]
    fn rotate_full_width_behaviour() {
        // Rotating by multiples of width should be identity
        let x64 = 0xDEADBEEFDEADBEEFu64;
        assert_eq!(rol_u64(x64, 64), x64);
        assert_eq!(ror_u64(x64, 64), x64);

        let x32 = 0xCAFEBABEu32;
        assert_eq!(rol_u32(x32, 32), x32);
        assert_eq!(ror_u32(x32, 32), x32);
    }
}
