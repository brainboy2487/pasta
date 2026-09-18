// src/ai/generate.rs
//! Lightweight generation utilities for PASTA AI components
//!
//! This module provides small, well-tested utilities for sampling and sequence
//! generation used by example models and tests. It intentionally avoids heavy
//! dependencies and focuses on clarity and reproducibility.
//!
//! Features:
//! - Softmax and numerically stable logit -> probability conversion.
//! - Sampling strategies: greedy, temperature sampling, top-k, nucleus (top-p).
//! - A small `generate_from_model` driver that repeatedly queries a user-provided
//!   model function for next-token logits and produces a token sequence.
//! - Uses the runtime `Rng` so hardware RNG is used when available.
//!
//! Notes:
//! - This is not a full language model runtime. `model_fn` is a user-supplied
//!   callback that maps a token prefix to a vector of logits for the next token.
//! - Token ids are `usize` for simplicity; adapt as needed for your tokenizer.

use anyhow::{anyhow, Result};
use std::cmp;
use std::f64;

use crate::runtime::rng::Rng;

/// Convert logits to probabilities using a numerically-stable softmax.
///
/// `logits` may be any finite f64 values. Returns a Vec<f64> of same length
/// summing to (approximately) 1.0.
pub fn softmax(logits: &[f64], temperature: f64) -> Vec<f64> {
    // Temperature scaling: divide logits by temperature (higher temp -> flatter)
    let temp = if temperature <= 0.0 { 1e-8 } else { temperature };
    let inv_temp = 1.0 / temp;

    // Find max for numerical stability
    let max_logit = logits
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    let mut exps: Vec<f64> = Vec::with_capacity(logits.len());
    let mut sum = 0.0f64;
    for &l in logits {
        let v = ((l - max_logit) * inv_temp).exp();
        exps.push(v);
        sum += v;
    }
    if sum == 0.0 {
        // fallback to uniform
        let n = logits.len() as f64;
        return vec![1.0 / n; logits.len()];
    }
    for v in exps.iter_mut() {
        *v /= sum;
    }
    exps
}

/// Apply top-k filtering to a probability vector in-place.
///
/// Keeps only the top `k` probabilities (by value) and renormalizes; if `k`
/// is 0 or >= vocab size, this is a no-op.
pub fn apply_top_k(probs: &mut [f64], k: usize) {
    if k == 0 || k >= probs.len() {
        return;
    }
    // Find threshold: the k-th largest probability
    // Use a simple partial selection via nth_element equivalent: clone and sort small vec
    let mut copy = probs.to_vec();
    copy.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let threshold = copy[k - 1];
    let mut sum = 0.0;
    for p in probs.iter_mut() {
        if *p < threshold {
            *p = 0.0;
        } else {
            sum += *p;
        }
    }
    if sum == 0.0 {
        // If everything zeroed (rare due to ties), fallback to uniform over top-k indices
        let mut indices: Vec<(usize, f64)> = probs
            .iter()
            .cloned()
            .enumerate()
            .collect();
        indices.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for i in 0..k {
            probs[indices[i].0] = 1.0 / (k as f64);
        }
        return;
    }
    for p in probs.iter_mut() {
        *p /= sum;
    }
}

/// Apply nucleus (top-p) filtering to a probability vector in-place.
///
/// Keeps the smallest set of highest-probability tokens whose cumulative
/// probability >= `p` and renormalizes. `p` should be in (0,1]. If `p` >= 1.0
/// this is a no-op.
pub fn apply_top_p(probs: &mut [f64], p: f64) {
    if p >= 1.0 || p <= 0.0 {
        return;
    }
    // Create index-prob pairs and sort descending
    let mut pairs: Vec<(usize, f64)> = probs
        .iter()
        .cloned()
        .enumerate()
        .collect();
    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut cum = 0.0;
    let mut keep = vec![false; probs.len()];
    for (idx, prob) in pairs.iter() {
        cum += *prob;
        keep[*idx] = true;
        if cum >= p {
            break;
        }
    }

    let mut sum = 0.0;
    for (i, pr) in probs.iter_mut().enumerate() {
        if !keep[i] {
            *pr = 0.0;
        } else {
            sum += *pr;
        }
    }
    if sum == 0.0 {
        // fallback: keep the highest-prob token
        let (best_idx, _) = pairs[0];
        for (i, pr) in probs.iter_mut().enumerate() {
            *pr = if i == best_idx { 1.0 } else { 0.0 };
        }
        return;
    }
    for pr in probs.iter_mut() {
        *pr /= sum;
    }
}

/// Sample an index from a discrete probability distribution `probs` using `rng`.
///
/// `probs` must sum to ~1.0. Returns `None` if `probs` is empty.
pub fn sample_from_probs(rng: &mut Rng, probs: &[f64]) -> Option<usize> {
    if probs.is_empty() {
        return None;
    }
    // Build cumulative distribution
    let mut cum = 0.0f64;
    let mut cdf: Vec<f64> = Vec::with_capacity(probs.len());
    for &p in probs {
        cum += p;
        cdf.push(cum);
    }
    // Ensure last element is exactly 1.0 to avoid floating point issues
    if let Some(last) = cdf.last_mut() {
        *last = 1.0;
    }
    let r = (rng.next_u64() as f64) / (u64::MAX as f64);
    // Binary search
    match cdf.binary_search_by(|v| v.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal)) {
        Ok(idx) => Some(idx),
        Err(idx) => Some(cmp::min(idx, probs.len() - 1)),
    }
}

/// Sampling configuration for generation.
#[derive(Debug, Clone, Copy)]
pub struct SamplingConfig {
    /// Temperature for softmax (1.0 = default).
    pub temperature: f64,
    /// Top-k filtering (0 = disabled).
    pub top_k: usize,
    /// Top-p (nucleus) filtering (1.0 = disabled).
    pub top_p: f64,
    /// If true, use greedy decoding (ignore sampling).
    pub greedy: bool,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            top_k: 0,
            top_p: 1.0,
            greedy: false,
        }
    }
}

/// Generate a sequence of token ids by repeatedly calling `model_fn`.
///
/// - `prefix` is the initial token sequence (may be empty).
/// - `steps` is the number of tokens to generate.
/// - `model_fn` is a callback `Fn(&[usize]) -> Result<Vec<f64>>` that returns
///   logits for the next token given the current prefix.
/// - `cfg` controls sampling behavior.
/// - `rng` is used for sampling (hardware RNG preferred).
///
/// Returns the full sequence (prefix followed by generated tokens).
pub fn generate_from_model<F>(
    mut prefix: Vec<usize>,
    steps: usize,
    mut model_fn: F,
    cfg: SamplingConfig,
    rng: &mut Rng,
) -> Result<Vec<usize>>
where
    F: FnMut(&[usize]) -> Result<Vec<f64>>,
{
    for _ in 0..steps {
        let logits = model_fn(&prefix)?;
        if logits.is_empty() {
            return Err(anyhow!("model returned empty logits"));
        }

        // Greedy: pick argmax
        if cfg.greedy {
            let mut best_idx = 0usize;
            let mut best_val = f64::NEG_INFINITY;
            for (i, &v) in logits.iter().enumerate() {
                if v > best_val {
                    best_val = v;
                    best_idx = i;
                }
            }
            prefix.push(best_idx);
            continue;
        }

        // Convert logits -> probs with temperature
        let mut probs = softmax(&logits, cfg.temperature);

        // Apply top-k then top-p (order matters; common practice is top-k then top-p)
        if cfg.top_k > 0 {
            apply_top_k(&mut probs, cfg.top_k);
        }
        if cfg.top_p < 1.0 {
            apply_top_p(&mut probs, cfg.top_p);
        }

        // Sample
        if let Some(idx) = sample_from_probs(rng, &probs) {
            prefix.push(idx);
        } else {
            // Fallback to argmax if sampling failed
            let mut best_idx = 0usize;
            let mut best_val = f64::NEG_INFINITY;
            for (i, &v) in probs.iter().enumerate() {
                if v > best_val {
                    best_val = v;
                    best_idx = i;
                }
            }
            prefix.push(best_idx);
        }
    }
    Ok(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::rng::Rng;

    /// Dummy model that returns logits favoring token (last_token + 1) mod V
    fn dummy_model(vocab: usize) -> impl Fn(&[usize]) -> Result<Vec<f64>> {
        move |prefix: &[usize]| {
            let mut logits = vec![0.0f64; vocab];
            let favored = if prefix.is_empty() { 0 } else { (prefix[prefix.len() - 1] + 1) % vocab };
            for i in 0..vocab {
                logits[i] = if i == favored { 5.0 } else { 0.0 };
            }
            Ok(logits)
        }
    }

    #[test]
    fn softmax_basic() {
        let logits = vec![0.0, 1.0, 2.0];
        let probs = softmax(&logits, 1.0);
        assert_eq!(probs.len(), 3);
        let sum: f64 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12);
    }

    #[test]
    fn top_k_and_p_behaviour() {
        let mut probs = vec![0.1, 0.2, 0.3, 0.4];
        apply_top_k(&mut probs, 2);
        let sum: f64 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12);
        // top-p: keep cumulative >= 0.6 -> should keep 0.4 and 0.3
        let mut probs2 = vec![0.1, 0.2, 0.3, 0.4];
        apply_top_p(&mut probs2, 0.6);
        let sum2: f64 = probs2.iter().sum();
        assert!((sum2 - 1.0).abs() < 1e-12);
    }

    #[test]
    fn sample_from_probs_nonempty() {
        let mut rng = Rng::new();
        let probs = vec![0.0, 0.0, 1.0];
        let idx = sample_from_probs(&mut rng, &probs).unwrap();
        assert_eq!(idx, 2);
    }

    #[test]
    fn generate_from_dummy_model_greedy() {
        let mut rng = Rng::new();
        let cfg = SamplingConfig {
            greedy: true,
            ..Default::default()
        };
        let model = dummy_model(5);
        let seq = generate_from_model(vec![], 4, model, cfg, &mut rng).unwrap();
        // Greedy picks favored token deterministically: 0,1,2,3,4 (starting from 0)
        assert_eq!(seq.len(), 4);
    }

    #[test]
    fn generate_from_dummy_model_sampling() {
        let mut rng = Rng::new();
        let cfg = SamplingConfig {
            temperature: 1.0,
            top_k: 0,
            top_p: 1.0,
            greedy: false,
        };
        let model = dummy_model(3);
        let seq = generate_from_model(vec![0], 3, model, cfg, &mut rng).unwrap();
        assert_eq!(seq.len(), 4); // prefix 1 + 3 generated
    }
}
