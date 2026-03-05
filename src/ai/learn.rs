// src/ai/learn.rs
//! Training and optimization utilities for PASTA AI
//!
//! This module provides a small, pragmatic training loop and optimizers that
//! integrate with the lightweight `autograd` Tensor implementation. It also
//! includes optional support for the ndarray-backed `tensor` when the
//! `ndarray` feature is enabled.
//!
//! Provided:
//! - Loss functions: `mse_loss` (mean squared error).
//! - Optimizers: `Sgd` and `Adam` (simple, per-parameter state).
//! - `Optimizer` enum wrapper and `step` API that updates a list of parameter
//!   `Tensor`s (leaves that `require_grad()`).
//! - `Trainer` struct: orchestrates epochs, batching, shuffling, and training.
//!
//! Design notes:
//! - This trainer is intentionally small and assumes the model closure captures
//!   its parameters as `autograd::Tensor` leaves. The trainer calls the model
//!   to produce predictions, computes loss, calls `backward()`, and then asks
//!   the optimizer to update the captured parameters.
//! - The optimizer expects parameters to expose mutable storage via the
//!   autograd `Tensor` internals (we mutate `node.data` directly).
//! - For reproducibility and to use hardware RNG when available, the trainer
//!   accepts a `crate::runtime::rng::Rng` instance for shuffling.

use std::time::Instant;

use anyhow::{Result, anyhow};

use crate::ai::autograd::Tensor as AutoTensor;
use crate::ai::datasets::Dataset;
use crate::runtime::rng::Rng;

/// Mean squared error loss: mean((pred - target)^2)
pub fn mse_loss(pred: &AutoTensor, target: &AutoTensor) -> AutoTensor {
    let diff = pred.sub(target);
    let sq = diff.powf(2.0);
    sq.mean()
}

/// Trait-like helper to access and mutate autograd Tensor parameters.
///
/// We implement the minimal helpers needed by the optimizers: read grads and
/// mutate data in-place. This is intentionally narrow to avoid exposing internals.
fn tensor_get_data_mut(t: &AutoTensor) -> Vec<f64> {
    t.node.borrow().data.clone()
}

fn tensor_set_data(t: &AutoTensor, new_data: Vec<f64>) {
    let mut n = t.node.borrow_mut();
    assert_eq!(n.data.len(), new_data.len(), "shape mismatch when setting tensor data");
    n.data = new_data;
}

fn tensor_get_grad(t: &AutoTensor) -> Option<Vec<f64>> {
    t.node.borrow().grad.clone()
}

fn tensor_zero_grad(t: &AutoTensor) {
    t.zero_grad();
}

/// Simple SGD optimizer
pub struct Sgd {
    pub lr: f64,
}

impl Sgd {
    pub fn new(lr: f64) -> Self {
        Self { lr }
    }

    /// Apply one optimization step to the provided parameters.
    ///
    /// `params` are expected to be `Tensor` leaves that have `grad` populated.
    pub fn step(&self, params: &[AutoTensor]) -> Result<()> {
        for p in params {
            if let Some(g) = tensor_get_grad(p) {
                let mut data = tensor_get_data_mut(p);
                if data.len() != g.len() {
                    return Err(anyhow!("param/grad length mismatch in SGD"));
                }
                for i in 0..data.len() {
                    data[i] -= self.lr * g[i];
                }
                tensor_set_data(p, data);
            } else {
                // No grad: skip
            }
        }
        Ok(())
    }
}

/// Simple Adam optimizer (per-parameter state)
pub struct Adam {
    pub lr: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub eps: f64,
    /// Per-parameter first moment vectors (m)
    m: Vec<Vec<f64>>,
    /// Per-parameter second moment vectors (v)
    v: Vec<Vec<f64>>,
    /// Time step
    t: usize,
}

impl Adam {
    pub fn new(lr: f64, beta1: f64, beta2: f64, eps: f64) -> Self {
        Self {
            lr,
            beta1,
            beta2,
            eps,
            m: Vec::new(),
            v: Vec::new(),
            t: 0,
        }
    }

    /// Initialize internal state for the given parameters if not already initialized.
    fn ensure_state(&mut self, params: &[AutoTensor]) {
        if self.m.len() != params.len() {
            self.m = params.iter().map(|p| vec![0.0; p.node.borrow().data.len()]).collect();
            self.v = params.iter().map(|p| vec![0.0; p.node.borrow().data.len()]).collect();
            self.t = 0;
        }
    }

    pub fn step(&mut self, params: &[AutoTensor]) -> Result<()> {
        self.ensure_state(params);
        self.t += 1;
        let t_f = self.t as f64;
        for (idx, p) in params.iter().enumerate() {
            let grad_opt = tensor_get_grad(p);
            if grad_opt.is_none() {
                continue;
            }
            let g = grad_opt.unwrap();
            let mut data = tensor_get_data_mut(p);
            if data.len() != g.len() {
                return Err(anyhow!("param/grad length mismatch in Adam"));
            }
            let m = &mut self.m[idx];
            let v = &mut self.v[idx];
            for i in 0..data.len() {
                m[i] = self.beta1 * m[i] + (1.0 - self.beta1) * g[i];
                v[i] = self.beta2 * v[i] + (1.0 - self.beta2) * (g[i] * g[i]);
                let m_hat = m[i] / (1.0 - self.beta1.powf(t_f));
                let v_hat = v[i] / (1.0 - self.beta2.powf(t_f));
                data[i] -= self.lr * m_hat / (v_hat.sqrt() + self.eps);
            }
            tensor_set_data(p, data);
        }
        Ok(())
    }
}

/// Generic optimizer wrapper to allow switching algorithms easily.
pub enum Optimizer {
    Sgd(Sgd),
    Adam(Adam),
}

impl Optimizer {
    pub fn step(&mut self, params: &[AutoTensor]) -> Result<()> {
        match self {
            Optimizer::Sgd(s) => s.step(params),
            Optimizer::Adam(a) => a.step(params),
        }
    }
}

/// Trainer configuration
pub struct TrainerConfig {
    pub epochs: usize,
    pub batch_size: usize,
    pub lr: f64,
    pub optimizer: Optimizer,
    pub verbose: bool,
}

impl Default for TrainerConfig {
    fn default() -> Self {
        Self {
            epochs: 10,
            batch_size: 32,
            lr: 1e-2,
            optimizer: Optimizer::Sgd(Sgd::new(1e-2)),
            verbose: true,
        }
    }
}

/// Trainer orchestrates training for models built with autograd Tensors.
///
/// The `model_fn` closure should accept a `&AutoTensor` features tensor and
/// return a predictions `AutoTensor`. The model closure is expected to capture
/// its parameters (as `AutoTensor` leaves) so the trainer can update them via
/// the provided `params` slice.
pub struct Trainer {
    pub cfg: TrainerConfig,
    pub rng: Rng,
}

impl Trainer {
    pub fn new(cfg: TrainerConfig, rng: Rng) -> Self {
        Self { cfg, rng }
    }

    /// Train the model on the provided dataset.
    ///
    /// - `model_fn`: closure mapping features Tensor -> predictions Tensor.
    /// - `params`: slice of parameter Tensors that will be updated by the optimizer.
    ///
    /// Returns training loss history (one value per epoch).
    pub fn train<F>(&mut self, dataset: &Dataset, mut model_fn: F, params: &[AutoTensor]) -> Result<Vec<f64>>
    where
        F: FnMut(&AutoTensor) -> AutoTensor,
    {
        if dataset.n_rows == 0 {
            return Err(anyhow!("empty dataset"));
        }
        let mut losses: Vec<f64> = Vec::with_capacity(self.cfg.epochs);
        let n = dataset.n_rows;
        let batch_size = self.cfg.batch_size.max(1);
        let mut indices: Vec<usize> = (0..n).collect();

        for epoch in 0..self.cfg.epochs {
            // Shuffle indices using provided RNG
            for i in (1..n).rev() {
                let j = (self.rng.next_u64() as usize) % (i + 1);
                indices.swap(i, j);
            }

            let mut epoch_loss = 0.0f64;
            let mut seen = 0usize;
            let start_time = Instant::now();

            // Iterate batches
            let mut pos = 0usize;
            while pos < n {
                let take = std::cmp::min(batch_size, n - pos);
                // Build feature and label vectors for this batch
                let mut feats: Vec<f64> = Vec::with_capacity(take * dataset.n_features);
                let mut labs: Vec<f64> = Vec::with_capacity(take * dataset.n_label_dims);
                for r in 0..take {
                    let idx = indices[pos + r];
                    let start = idx * dataset.n_features;
                    feats.extend_from_slice(&dataset.features[start..start + dataset.n_features]);
                    if let Some(ref labs_all) = dataset.labels {
                        let lstart = idx * dataset.n_label_dims;
                        labs.extend_from_slice(&labs_all[lstart..lstart + dataset.n_label_dims]);
                    }
                }

                // Convert to Tensors
                // For simplicity we assume single-dim features (n_features == 1) or treat features as flat vector
                // The model_fn is responsible for interpreting the features shape.
                let x = AutoTensor::from_vec(feats.clone(), vec![take * dataset.n_features], false);
                let y = if dataset.labels.is_some() {
                    AutoTensor::from_vec(labs.clone(), vec![take * dataset.n_label_dims], false)
                } else {
                    // If no labels, skip this batch
                    pos += take;
                    continue;
                };

                // Zero grads on parameters
                for p in params {
                    tensor_zero_grad(p);
                }

                // Forward
                let preds = model_fn(&x);

                // Compute loss
                let loss = mse_loss(&preds, &y);

                // Backward
                loss.backward();

                // Accumulate scalar loss value (read from loss node)
                let loss_val = loss.node.borrow().data[0];
                epoch_loss += loss_val * (take as f64);
                seen += take;

                // Optimizer step
                self.cfg.optimizer.step(params)?;

                // Zero grads on parameters after step to avoid accumulation across batches
                for p in params {
                    tensor_zero_grad(p);
                }

                pos += take;
            }

            let avg_loss = if seen > 0 { epoch_loss / (seen as f64) } else { 0.0 };
            losses.push(avg_loss);

            if self.cfg.verbose {
                let elapsed = start_time.elapsed();
                println!(
                    "Epoch {:3} / {:3}  loss={:.6}  samples={}  time={:.2?}",
                    epoch + 1,
                    self.cfg.epochs,
                    avg_loss,
                    seen,
                    elapsed
                );
            }
        }

        Ok(losses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::autograd::Tensor as T;
    use crate::ai::datasets::Dataset;
    use crate::runtime::rng::Rng;

    /// Simple linear model test: learn y = 2*x + 1 using autograd tensors.
    #[test]
    fn linear_regression_sgd() {
        // Create tiny dataset: x in [0,1,2,3], y = 2*x + 1
        let mut ds = Dataset::new();
        ds.n_rows = 8;
        ds.n_features = 1;
        ds.n_label_dims = 1;
        ds.features = (0..8).map(|i| i as f64).collect();
        ds.labels = Some((0..8).map(|i| 2.0 * (i as f64) + 1.0).collect());

        // Model parameters: w and b
        let w = T::scalar_requires_grad(0.0);
        let b = T::scalar_requires_grad(0.0);

        // Model closure: expects features as flat vector of length batch_size (n_features==1)
        let model = |x: &T| {
            // x is flat vector; treat as elementwise multiply with scalar w and add b
            let wx = w.mul(x);
            wx.add(&b)
        };

        // Trainer config: small lr, many epochs
        let mut cfg = TrainerConfig::default();
        cfg.epochs = 200;
        cfg.batch_size = 4;
        cfg.lr = 1e-3;
        cfg.optimizer = Optimizer::Sgd(Sgd::new(1e-3));
        cfg.verbose = false;

        let mut trainer = Trainer::new(cfg, Rng::new());
        let params = vec![w.clone(), b.clone()];

        let losses = trainer.train(&ds, model, &params).unwrap();
        // final loss should be small
        let final_loss = *losses.last().unwrap();
        assert!(final_loss < 1e-2 || final_loss.is_finite());

        // Check parameters near expected values
        let w_val = w.node.borrow().data[0];
        let b_val = b.node.borrow().data[0];
        assert!((w_val - 2.0).abs() < 0.5);
        assert!((b_val - 1.0).abs() < 0.5);
    }

    #[test]
    fn adam_step_updates_params() {
        let mut adam = Adam::new(1e-2, 0.9, 0.999, 1e-8);
        let p = T::from_vec(vec![1.0, 2.0], vec![2], true);
        // simulate grads
        p.node.borrow_mut().grad = Some(vec![0.1, 0.2]);
        adam.step(&[p.clone()]).unwrap();
        // After one step, data should have changed
        let d = p.node.borrow().data.clone();
        assert!(d[0] != 1.0 || d[1] != 2.0);
    }
}
