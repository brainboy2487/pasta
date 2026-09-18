use std::sync::RwLock;
use once_cell::sync::Lazy;

/// Default epsilon used internally for tiny comparisons when needed.
pub const DEFAULT_FLOAT_DOWNCAST_EPS: f64 = 1e-12;

/// Global override (optional). Use RwLock for thread-safety.
static GLOBAL_FLOAT_DOWNCAST_EPS: Lazy<RwLock<f64>> = Lazy::new(|| RwLock::new(DEFAULT_FLOAT_DOWNCAST_EPS));

/// Set a global epsilon. Prefer per-engine config for deterministic behavior in production.
pub fn set_global_float_downcast_eps(eps: f64) {
    let mut g = GLOBAL_FLOAT_DOWNCAST_EPS.write().unwrap();
    *g = eps;
}

/// Read the global epsilon.
pub fn get_global_float_downcast_eps() -> f64 {
    *GLOBAL_FLOAT_DOWNCAST_EPS.read().unwrap()
}

/// Decide if a float is integral within eps and fits in i64.
pub fn float_to_int_if_integral_with_eps(v: f64, eps: f64) -> Option<i64> {
    if !v.is_finite() { return None; }
    let r = v.round();
    if (v - r).abs() <= eps {
        if r >= (i64::MIN as f64) && r <= (i64::MAX as f64) {
            return Some(r as i64);
        }
    }
    None
}

/// Convenience that uses the global eps.
pub fn float_to_int_if_integral(v: f64) -> Option<i64> {
    float_to_int_if_integral_with_eps(v, get_global_float_downcast_eps())
}

/// Rounding mode enum mirrored in types.rs; keep in sync.
#[derive(Debug, Clone, Copy)]
pub enum FloatRoundingMode {
    RoundHalfEven,
    RoundHalfAwayFromZero,
    Truncate,
}

/// Round a float according to the tolerance level and rounding mode.
/// Level 1 -> decimals = 0; Level N -> decimals = N-1 for N>=2.
/// Returns None for NaN/Inf.
pub fn round_with_level(v: f64, level: u8, mode: FloatRoundingMode) -> Option<f64> {
    if !v.is_finite() { return None; }
    let level = if level == 0 { 1 } else { level.min(20) };
    let decimals = (level.saturating_sub(1)) as i32;
    let factor = 10f64.powi(decimals);
    let scaled = v * factor;

    let rounded_scaled = match mode {
        FloatRoundingMode::RoundHalfEven => scaled.round(),
        FloatRoundingMode::RoundHalfAwayFromZero => {
            // implement ties-away-from-zero for .5 cases
            let frac = scaled.fract();
            if frac.abs() == 0.5 {
                if scaled.is_sign_positive() { scaled.ceil() } else { scaled.floor() }
            } else {
                scaled.round()
            }
        }
        FloatRoundingMode::Truncate => scaled.trunc(),
    };

    Some(rounded_scaled / factor)
}

/// Format float according to level: level 1 -> no decimals, level N -> N-1 decimals.
pub fn format_with_level(v: f64, level: u8) -> String {
    if !v.is_finite() {
        return format!("{:?}", v);
    }
    let level = if level == 0 { 1 } else { level.min(20) };
    let decimals = (level.saturating_sub(1)) as usize;
    if decimals == 0 {
        format!("{:.0}", v)
    } else {
        format!("{:.*}", decimals, v)
    }
}
