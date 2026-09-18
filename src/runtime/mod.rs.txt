// src/runtime/mod.rs
//! Runtime utilities for PASTA
//!
//! This module groups small runtime subsystems:
//! - `asm`: sandboxed inline ASM blocks and executor
//! - `bitwise`: bitwise primitives (optional asm-backed on x86_64)
//! - `devices`: device architecture registry and auto-configuration
//!
//! Public API re-exports the primary types used by the interpreter and other
//! runtime components so callers can `use crate::runtime::*`.

pub mod asm;
pub mod bitwise;
pub mod devices;
pub mod meatball;
pub mod strainer;
pub mod rng;

pub use asm::{AsmBlock, AsmRuntime};
pub use bitwise::{
    and_u32, and_u64, or_u32, or_u64, xor_u32, xor_u64, not_u32, not_u64, shl_u32, shl_u64,
    shr_u32, shr_u64, rol_u32, rol_u64, ror_u32, ror_u64,
};
pub use devices::{auto_configure, detect_host_arch, find_device_by_id, find_device_for_current_host};
pub use meatball::{Meatball, MeatballConfig, MeatballHandle, Saucepan, Message, MeatballID};
pub use strainer::{Strainer, GcRef};
pub use rng::Rng;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::environment::Environment;
    use crate::interpreter::environment::Value;

    #[test]
    fn runtime_mod_smoke() {
        // Bitwise smoke
        assert_eq!(and_u64(0b11, 0b10), 0b10);
        assert_eq!(or_u32(0b01, 0b10), 0b11);
        assert_eq!(xor_u64(0xff, 0x0f), 0xf0);

        // ASM runtime smoke: create a block and execute against an environment
        let mut env = Environment::new();
        let block = AsmBlock::new(vec![
            "set x = 42".into(),
            "set msg = \"hi\"".into(),
        ]);
        let rt = AsmRuntime::new();
        rt.execute_block(&block, &mut env).unwrap();
        assert_eq!(env.get("x"), Some(Value::Number(42.0)));
        assert_eq!(env.get("msg"), Some(Value::String("hi".into())));

        // Devices smoke: attempt to auto-configure (may return None on unknown arch)
        let mut env2 = Environment::new();
        let res = auto_configure(&mut env2);
        assert!(res.is_ok());
    }

    #[test]
    fn device_detection_returns_string() {
        let arch = detect_host_arch();
        assert!(!arch.is_empty());
    }

    #[test]
    fn find_device_by_id_known() {
        let maybe = find_device_by_id("intel_x86_64").unwrap();
        assert!(maybe.is_some());
        let dev = maybe.unwrap();
        assert_eq!(dev.id, "intel_x86_64");
    }
}
