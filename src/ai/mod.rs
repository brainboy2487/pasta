// src/ai/mod.rs
//! AI utilities for PASTA
//!
//! This module groups small AI-related subsystems used by examples and tests:
//! - `autograd` — lightweight, dependency-free autodiff engine (pure Rust).
//! - `tensor` — ndarray-backed tensor utilities (optional, requires `ndarray`).
//! - `datasets` — dataset loading, preprocessing, batching utilities.
//! - `generate` — sampling and sequence generation helpers.
//!
//! The crate exposes a compact, ergonomic surface for small experiments and
//! examples. Use `autograd` for minimal, dependency-free work and `tensor` when
//! you want `ndarray` performance and broadcasting support.

pub mod autograd;
#[cfg(feature = "ndarray")]
pub mod tensor;
pub mod datasets;
pub mod generate;

pub use autograd::Tensor as AutoTensor;
#[cfg(feature = "ndarray")]
pub use tensor::Tensor as NdTensor;
pub use datasets::{Dataset, Normalization};
pub use generate::{generate_from_model, SamplingConfig, softmax};

/// Convenience: create a small toy regression dataset (y = a*x + b + noise)
///
/// - `n` rows, `seed` for RNG, `noise_std` standard deviation of Gaussian noise.
/// - Returns a `Dataset` with one feature and one label.
pub fn make_linear_toy(n: usize, a: f64, b: f64, noise_std: f64, seed: u64) -> Dataset {
    let mut ds = Dataset::new();
    ds.n_rows = n;
    ds.n_features = 1;
    ds.n_label_dims = 1;
    ds.feature_names = vec!["x".into()];
    ds.label_names = vec!["y".into()];
    ds.features = Vec::with_capacity(n * 1);
    ds.labels = Some(Vec::with_capacity(n * 1));

    // Simple xorshift64* seeded generator for reproducibility
    let mut s = if seed == 0 { 0xDEADBEEFCAFEBABEu64 } else { seed };
    for i in 0..n {
        // deterministic pseudo-random in [0,1)
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        let rnd = (s.wrapping_mul(0x2545F4914F6CDD1Du64) as f64) / (u64::MAX as f64);
        let x = (i as f64) * 1.0 + rnd;
        let mut noise = 0.0;
        if noise_std > 0.0 {
            // simple scaled pseudo-normal via Box-Muller using two xorshift draws
            s ^= s >> 12;
            s ^= s << 25;
            s ^= s >> 27;
            let u1 = ((s.wrapping_mul(0x2545F4914F6CDD1Du64) as f64) / (u64::MAX as f64)).max(1e-12);
            s ^= s >> 12;
            s ^= s << 25;
            s ^= s >> 27;
            let u2 = ((s.wrapping_mul(0x2545F4914F6CDD1Du64) as f64) / (u64::MAX as f64)).max(1e-12);
            let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            noise = z0 * noise_std;
        }
        let y = a * x + b + noise;
        ds.features.push(x);
        ds.labels.as_mut().unwrap().push(y);
    }

    ds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::rng::Rng;

    #[test]
    fn make_linear_toy_basic() {
        let ds = make_linear_toy(10, 2.0, 1.0, 0.0, 42);
        assert_eq!(ds.n_rows, 10);
        assert_eq!(ds.n_features, 1);
        assert_eq!(ds.n_label_dims, 1);
        // simple linear relation without noise: y == 2*x + 1
        for i in 0..ds.n_rows {
            let x = ds.features[i];
            let y = ds.labels.as_ref().unwrap()[i];
            assert!((y - (2.0 * x + 1.0)).abs() < 1e-8);
        }
    }

    #[test]
    fn dataset_shuffle_and_batches() {
        let mut ds = make_linear_toy(20, 1.0, 0.0, 0.1, 7);
        let mut rng = Rng::new();
        ds.shuffle_inplace(&mut rng);
        let mut it = ds.batches(5);
        let first = it.next().unwrap();
        assert_eq!(first.0.len(), 5 * ds.n_features);
    }

    #[cfg(feature = "ndarray")]
    #[test]
    fn ndarray_tensor_smoke() {
        use crate::ai::NdTensor;
        let a = NdTensor::from_vec(vec![1.0, 2.0, 3.0], &[3], true);
        let b = NdTensor::from_vec(vec![4.0, 5.0, 6.0], &[3], true);
        let c = (&a).add(&b);
        c.backward();
        let ag = a.node.borrow().grad.as_ref().unwrap().clone();
        assert_eq!(ag.iter().cloned().collect::<Vec<_>>(), vec![1.0, 1.0, 1.0]);
    }
}
