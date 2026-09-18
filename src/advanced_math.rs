// src/advanced_math.rs

use num_bigint::BigInt;
use num_traits::{One, Zero};

/// Fast Quake-style inverse square root for f32 values with one Newton iteration.
pub fn inv_sqrt_fast_f32(x: f32) -> f32 {
    if x <= 0.0 {
        return 1.0 / x.sqrt();
    }
    let xhalf = 0.5f32 * x;
    let mut i: u32 = x.to_bits();
    i = 0x5f3759dfu32.wrapping_sub(i >> 1);
    let mut y = f32::from_bits(i);
    // One Newton iteration for improved accuracy
    y = y * (1.5 - xhalf * y * y);
    y
}

/// Compute tetration `base ^^ height` for non-negative integer `height`.
/// Uses f64 arithmetic and right-associative tower evaluation. Returns NaN/Inf
/// if the result overflows f64.
pub fn tetration(base: f64, height: usize) -> f64 {
    if height == 0 { return 1.0; }
    let mut r = base;
    for _ in 1..height {
        r = base.powf(r);
        if r.is_nan() || r.is_infinite() { break; }
    }
    r
}

fn factorial(n: usize) -> BigInt {
    let mut f = BigInt::one();
    for i in 2..=n {
        f *= i;
    }
    f
}

fn isqrt(n: &BigInt) -> BigInt {
    if n.is_zero() { return BigInt::zero(); }
    let two = BigInt::from(2u32);
    let mut x = n.clone();
    loop {
        let y = (&x + n / &x) / &two;
        if y >= x { return x; }
        x = y;
    }
}

/// Compute pi to `digits` decimal places using the Chudnovsky algorithm.
/// Returns a decimal string like "3.14159...". Supports moderately large
/// `digits` (hundreds to low thousands) but is not optimized for extreme sizes.
pub fn calc_pi(digits: usize) -> String {
    // Extra guard digits for rounding safety.
    let extra = 10usize;
    let max_digits = digits + extra;

    let scale = BigInt::from(10u32).pow(max_digits as u32);

    // Number of terms needed: Chudnovsky gives ~14.181647 digits per term
    let terms = (digits as f64 / 14.181647462725477_f64) as usize + 1;

    let mut sum = BigInt::zero();
    let big_640320 = BigInt::from(640320u32);

    for k in 0..terms {
        let six_k = 6 * k;
        let three_k = 3 * k;
        let fk6 = factorial(six_k);
        let fk3 = factorial(three_k);
        let fk = factorial(k);
        let big_coeff = BigInt::from(13591409u64 + 545140134u64 * (k as u64));
        let numerator = fk6 * big_coeff;
        let denom = fk3 * fk.pow(3u32) * big_640320.pow((3 * k) as u32);
        let term = (&numerator * &scale) / &denom;
        if k % 2 == 0 {
            sum += term;
        } else {
            sum -= term;
        }
    }

    let sqrt_scaled = isqrt(&(&BigInt::from(10005u32) * &scale * &scale));
    let numerator = BigInt::from(426880u32) * sqrt_scaled;
    let pi_scaled = &numerator / &sum; // scaled by 10^(digits+extra)

    // Round to requested digits
    let round_div = BigInt::from(10u32).pow(extra as u32);
    let pi_rounded = (&pi_scaled + &round_div / 2u32) / &round_div; // scaled by 10^digits

    let ten_pow_digits = BigInt::from(10u32).pow(digits as u32);
    let int_part = &pi_rounded / &ten_pow_digits;
    let frac_part = &pi_rounded % &ten_pow_digits;

    let int_str = int_part.to_string();
    let mut frac_str = frac_part.to_string();
    while frac_str.len() < digits { frac_str = format!("0{}", frac_str); }
    format!("{}.{}", int_str, frac_str)
}