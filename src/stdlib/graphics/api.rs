//! Stateless graphics helper builtins used by `FROM graphics USE ...`.
//!
//! Live window/canvas state is owned by [`crate::interpreter::Executor`].
//! This module only contains pure value helpers that do not need executor access.

use anyhow::{anyhow, Result};

use crate::interpreter::Value;

fn get_num(args: &[Value], idx: usize, name: &str) -> Result<f64> {
    match args.get(idx) {
        Some(Value::Number(n)) => Ok(*n),
        _ => Err(anyhow!("{} must be a number", name)),
    }
}

/// color_rgb(r, g, b) -> packed 0xFFRRGGBB number.
pub fn color_rgb(args: Vec<Value>) -> Result<Value> {
    if args.len() != 3 {
        return Err(anyhow!("color_rgb(r, g, b)"));
    }
    let r = get_num(&args, 0, "r")? as u8;
    let g = get_num(&args, 1, "g")? as u8;
    let b = get_num(&args, 2, "b")? as u8;
    Ok(Value::Number(
        crate::stdlib::graphics::draw::color_rgb(r, g, b) as f64,
    ))
}

/// color_rgba(r, g, b, a) -> packed 0xAARRGGBB number.
pub fn color_rgba(args: Vec<Value>) -> Result<Value> {
    if args.len() != 4 {
        return Err(anyhow!("color_rgba(r, g, b, a)"));
    }
    let r = get_num(&args, 0, "r")? as u8;
    let g = get_num(&args, 1, "g")? as u8;
    let b = get_num(&args, 2, "b")? as u8;
    let a = get_num(&args, 3, "a")? as u8;
    Ok(Value::Number(
        crate::stdlib::graphics::draw::color_rgba(r, g, b, a) as f64,
    ))
}

/// color_hsv(h, s, v) -> packed 0xFFRRGGBB number.
pub fn color_hsv(args: Vec<Value>) -> Result<Value> {
    if args.len() != 3 {
        return Err(anyhow!("color_hsv(h, s, v)"));
    }
    let h = get_num(&args, 0, "h")?;
    let s = get_num(&args, 1, "s")?;
    let v = get_num(&args, 2, "v")?;
    Ok(Value::Number(
        crate::stdlib::graphics::draw::color_hsv(h, s, v) as f64,
    ))
}

/// color_lerp(c1, c2, t) -> interpolated packed color.
pub fn color_lerp(args: Vec<Value>) -> Result<Value> {
    if args.len() != 3 {
        return Err(anyhow!("color_lerp(c1, c2, t)"));
    }
    let c1 = get_num(&args, 0, "c1")? as u32;
    let c2 = get_num(&args, 1, "c2")? as u32;
    let t = get_num(&args, 2, "t")?;
    Ok(Value::Number(
        crate::stdlib::graphics::draw::color_lerp(c1, c2, t) as f64,
    ))
}

/// color_rgb16(r5, g6, b5) -> packed 0xFFRRGGBB number.
pub fn color_rgb16(args: Vec<Value>) -> Result<Value> {
    if args.len() != 3 {
        return Err(anyhow!("color_rgb16(r5, g6, b5)"));
    }
    let r = get_num(&args, 0, "r5")? as u8;
    let g = get_num(&args, 1, "g6")? as u8;
    let b = get_num(&args, 2, "b5")? as u8;
    Ok(Value::Number(
        crate::stdlib::graphics::draw::color_rgb16(r, g, b) as f64,
    ))
}

/// color_from565(packed16) -> packed 0xFFRRGGBB number.
pub fn color_from565(args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(anyhow!("color_from565(packed16)"));
    }
    let p = get_num(&args, 0, "packed16")? as u16;
    Ok(Value::Number(
        crate::stdlib::graphics::draw::color_from565(p) as f64,
    ))
}

/// color_to565(color) -> packed RGB565 number.
pub fn color_to565(args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(anyhow!("color_to565(color)"));
    }
    let c = get_num(&args, 0, "color")? as u32;
    Ok(Value::Number(
        crate::stdlib::graphics::draw::color_to565(c) as f64
    ))
}

/// color_wheel(angle) -> packed rainbow color.
pub fn color_wheel(args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(anyhow!("color_wheel(angle)"));
    }
    let a = get_num(&args, 0, "angle")?;
    Ok(Value::Number(
        crate::stdlib::graphics::draw::color_wheel(a) as f64
    ))
}

/// palette565_size() -> 65536.
pub fn palette565_size(args: Vec<Value>) -> Result<Value> {
    if !args.is_empty() {
        return Err(anyhow!("palette565_size() takes no arguments"));
    }
    Ok(Value::Number(
        crate::stdlib::graphics::draw::palette565_size() as f64,
    ))
}
