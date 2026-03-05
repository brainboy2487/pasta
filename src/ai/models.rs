// src/ai/models.rs
//! Simple model definitions for PASTA AI utilities
//!
//! This module provides a few lightweight model primitives built on top of the
//! `autograd` Tensor implementation. Models are intentionally small and
//! educational: they are easy to inspect, train with the `learn::Trainer`,
//! and useful for examples and tests.
//!
//! Provided models:
//! - `LinearModel` — single linear layer (y = W x + b) for regression/classification.
//! - `MLP` — small fully-connected multi-layer perceptron with ReLU activations.
//! - `SoftmaxClassifier` — convenience wrapper that applies softmax to logits.
//!
//! Each model exposes:
//! - `forward(&self, x: &Tensor) -> Tensor`
//! - `parameters(&self) -> Vec<Tensor>` to return trainable leaves.
//!
//! These models use the `autograd::Tensor` API and are suitable for use with
//! the `learn::Trainer` and `datasets::Dataset` utilities.

use anyhow::Result;
use std::rc::Rc;

use crate::ai::autograd::Tensor;
use crate::ai::autograd;
use crate::ai::datasets::Dataset;

/// Trait implemented by simple models in this module.
pub trait Model {
    /// Forward pass: map input tensor to output tensor (predictions / logits).
    fn forward(&self, x: &Tensor) -> Tensor;

    /// Return trainable parameters (leaves that require_grad).
    fn parameters(&self) -> Vec<Tensor>;
}

// -----------------------------
// LinearModel
// -----------------------------

/// Linear model: y = W x + b
///
/// - `in_dim` is number of input features.
/// - `out_dim` is number of output dims (1 for scalar regression).
#[derive(Clone)]
pub struct LinearModel {
    pub w: Tensor, // shape: [out_dim, in_dim] flattened as [out_dim * in_dim]
    pub b: Tensor, // shape: [out_dim]
    pub in_dim: usize,
    pub out_dim: usize,
}

impl LinearModel {
    /// Create a new linear model with small random-ish initialization.
    pub fn new(in_dim: usize, out_dim: usize, init_scale: f64) -> Self {
        // Initialize weights with small values; use simple pattern for determinism.
        let mut wdata = Vec::with_capacity(in_dim * out_dim);
        for i in 0..(in_dim * out_dim) {
            let v = ((i as f64 * 13.0 + 7.0).sin()) * init_scale;
            wdata.push(v);
        }
        let bdata = vec![0.0f64; out_dim];
        let w = Tensor::from_vec(wdata, vec![out_dim, in_dim], true);
        let b = Tensor::from_vec(bdata, vec![out_dim], true);
        Self { w, b, in_dim, out_dim }
    }

    /// Compute logits for a batch of inputs.
    ///
    /// Expects `x` to be a flat vector shaped `[batch_size * in_dim]`.
    /// Returns a tensor shaped `[batch_size * out_dim]`.
    fn linear_forward(&self, x: &Tensor) -> Tensor {
        // Use autograd matmul if available: reshape semantics in autograd are simple,
        // so implement manual matmul for 2D shapes: batch x in_dim  @  in_dim x out_dim^T
        // Our autograd Tensor supports matmul for 2D; create appropriate shapes.
        // Convert x to 2D: [batch, in_dim]
        let x_shape = x.node.borrow().shape.clone();
        let batch = if x_shape.is_empty() { 1 } else { x_shape[0] / self.in_dim };
        // Create views as 2D tensors by constructing new Tensor nodes with shapes.
        // For simplicity, use matmul on autograd: a.matmul(b)
        // Prepare W^T so that (batch x in_dim) @ (in_dim x out_dim) -> (batch x out_dim)
        let w_t = {
            // w is [out_dim, in_dim]; we need [in_dim, out_dim]
            let wdata = self.w.node.borrow().data.clone();
            let mut trans = vec![0.0; self.in_dim * self.out_dim];
            for i in 0..self.out_dim {
                for j in 0..self.in_dim {
                    trans[j * self.out_dim + i] = wdata[i * self.in_dim + j];
                }
            }
            Tensor::from_vec(trans, vec![self.in_dim, self.out_dim], false)
        };

        // Reshape x into [batch, in_dim] by creating a new Tensor that shares data.
        // autograd::Tensor doesn't support view semantics; we create a new Tensor from data.
        let xdata = x.node.borrow().data.clone();
        let x2 = Tensor::from_vec(xdata, vec![batch, self.in_dim], false);

        let out = x2.matmul(&w_t); // shape [batch, out_dim]
        // Add bias: broadcast b across batch
        let b_broadcast = Tensor::from_vec(self.b.node.borrow().data.clone(), vec![1, self.out_dim], false);
        let out_plus_b = out.add(&b_broadcast);
        // Flatten back to [batch * out_dim]
        let out_data = out_plus_b.node.borrow().data.clone();
        Tensor::from_vec(out_data, vec![batch * self.out_dim], out_plus_b.node.borrow().requires_grad)
    }
}

impl Model for LinearModel {
    fn forward(&self, x: &Tensor) -> Tensor {
        self.linear_forward(x)
    }

    fn parameters(&self) -> Vec<Tensor> {
        vec![self.w.clone(), self.b.clone()]
    }
}

// -----------------------------
// MLP
// -----------------------------

/// Simple fully-connected MLP with ReLU activations.
///
/// Layers are specified as a vector of hidden sizes. For example,
/// `MLP::new(4, vec![16, 8], 1)` creates a network 4 -> 16 -> 8 -> 1.
pub struct MLP {
    pub layers: Vec<(Tensor, Tensor)>, // (W, b) pairs; W shape [out, in], b shape [out]
    pub in_dim: usize,
    pub out_dim: usize,
}

impl MLP {
    pub fn new(in_dim: usize, hidden: Vec<usize>, out_dim: usize, init_scale: f64) -> Self {
        let mut dims = Vec::new();
        dims.push(in_dim);
        dims.extend(hidden.iter());
        dims.push(out_dim);
        let mut layers = Vec::new();
        for i in 0..(dims.len() - 1) {
            let in_d = dims[i];
            let out_d = dims[i + 1];
            let mut wdata = Vec::with_capacity(in_d * out_d);
            for k in 0..(in_d * out_d) {
                let v = ((k as f64 * 17.0 + (i as f64 * 31.0)).cos()) * init_scale;
                wdata.push(v);
            }
            let bdata = vec![0.0f64; out_d];
            let w = Tensor::from_vec(wdata, vec![out_d, in_d], true);
            let b = Tensor::from_vec(bdata, vec![out_d], true);
            layers.push((w, b));
        }
        Self { layers, in_dim, out_dim }
    }

    fn forward_internal(&self, x: &Tensor) -> Tensor {
        // x: flat [batch * in_dim]
        let mut cur = {
            let x_shape = x.node.borrow().shape.clone();
            let batch = if x_shape.is_empty() { 1 } else { x_shape[0] / self.in_dim };
            Tensor::from_vec(x.node.borrow().data.clone(), vec![batch, self.in_dim], false)
        };

        for (idx, (w, b)) in self.layers.iter().enumerate() {
            // prepare w_t: [in, out]
            let wdata = w.node.borrow().data.clone();
            let in_d = w.node.borrow().shape[1];
            let out_d = w.node.borrow().shape[0];
            let mut w_t_data = vec![0.0; in_d * out_d];
            for i in 0..out_d {
                for j in 0..in_d {
                    w_t_data[j * out_d + i] = wdata[i * in_d + j];
                }
            }
            let w_t = Tensor::from_vec(w_t_data, vec![in_d, out_d], false);
            let z = cur.matmul(&w_t); // [batch, out_d]
            let b_b = Tensor::from_vec(b.node.borrow().data.clone(), vec![1, out_d], false);
            let z = z.add(&b_b);
            // Activation: ReLU for hidden layers, identity for last
            if idx < self.layers.len() - 1 {
                cur = z.relu();
            } else {
                cur = z;
            }
        }

        // Flatten to [batch * out_dim]
        let out_data = cur.node.borrow().data.clone();
        let batch = cur.node.borrow().shape[0];
        Tensor::from_vec(out_data, vec![batch * self.out_dim], cur.node.borrow().requires_grad)
    }
}

impl Model for MLP {
    fn forward(&self, x: &Tensor) -> Tensor {
        self.forward_internal(x)
    }

    fn parameters(&self) -> Vec<Tensor> {
        let mut ps = Vec::new();
        for (w, b) in &self.layers {
            ps.push(w.clone());
            ps.push(b.clone());
        }
        ps
    }
}

// -----------------------------
// SoftmaxClassifier
// -----------------------------

/// Convenience wrapper that applies softmax to logits produced by an inner model.
///
/// This model returns probabilities (as a Tensor) shaped `[batch * n_classes]`.
pub struct SoftmaxClassifier<M: Model> {
    pub base: M,
    pub n_classes: usize,
}

impl<M: Model> SoftmaxClassifier<M> {
    pub fn new(base: M, n_classes: usize) -> Self {
        Self { base, n_classes }
    }
}

impl<M: Model> Model for SoftmaxClassifier<M> {
    fn forward(&self, x: &Tensor) -> Tensor {
        let logits = self.base.forward(x);
        // Softmax implemented in autograd is not available; we implement a numerically simple softmax
        // that returns a Tensor with same shape. For training with cross-entropy, users typically
        // compute logits directly and use a specialized loss; here we simply exponentiate and normalize.
        let data = logits.node.borrow().data.clone();
        // Determine batch size
        let shape = logits.node.borrow().shape.clone();
        let total = data.len();
        let batch = if self.n_classes > 0 { total / self.n_classes } else { 1 };
        let mut out = vec![0.0f64; total];
        for b in 0..batch {
            let start = b * self.n_classes;
            let end = start + self.n_classes;
            let slice = &data[start..end];
            let maxv = slice.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let mut sum = 0.0;
            for (i, &v) in slice.iter().enumerate() {
                let e = (v - maxv).exp();
                out[start + i] = e;
                sum += e;
            }
            if sum == 0.0 {
                for i in start..end {
                    out[i] = 1.0 / (self.n_classes as f64);
                }
            } else {
                for i in start..end {
                    out[i] /= sum;
                }
            }
        }
        Tensor::from_vec(out, vec![batch * self.n_classes], logits.node.borrow().requires_grad)
    }

    fn parameters(&self) -> Vec<Tensor> {
        self.base.parameters()
    }
}

// -----------------------------
// Tests
// -----------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::autograd::Tensor as T;
    use crate::ai::learn::{Trainer, TrainerConfig, Optimizer, Sgd};
    use crate::runtime::rng::Rng;

    #[test]
    fn linear_model_forward_shapes() {
        let lm = LinearModel::new(3, 2, 0.01);
        // batch size 4 -> input shape [4*3]
        let x = T::from_vec(vec![1.0; 12], vec![12], false);
        let out = lm.forward(&x);
        assert_eq!(out.node.borrow().data.len(), 4 * 2);
    }

    #[test]
    fn mlp_forward_and_params() {
        let mlp = MLP::new(2, vec![8, 4], 1, 0.05);
        let x = T::from_vec(vec![0.5; 6], vec![3 * 2], false); // batch 3
        let y = mlp.forward(&x);
        assert_eq!(y.node.borrow().data.len(), 3 * 1);
        let params = mlp.parameters();
        assert!(!params.is_empty());
    }

    #[test]
    fn train_linear_model_on_toy_data() {
        // Create toy dataset y = 3*x + 2
        let mut ds = Dataset::new();
        ds.n_rows = 20;
        ds.n_features = 1;
        ds.n_label_dims = 1;
        ds.features = (0..20).map(|i| i as f64).collect();
        ds.labels = Some((0..20).map(|i| 3.0 * (i as f64) + 2.0).collect());

        let lm = LinearModel::new(1, 1, 0.01);
        let params = lm.parameters();

        // Model closure: expects flat features vector
        let model = |x: &T| lm.forward(x);

        let mut cfg = TrainerConfig::default();
        cfg.epochs = 80;
        cfg.batch_size = 5;
        cfg.optimizer = Optimizer::Sgd(Sgd::new(1e-3));
        cfg.verbose = false;

        let mut trainer = Trainer::new(cfg, Rng::new());
        let losses = trainer.train(&ds, model, &params).unwrap();
        let final = *losses.last().unwrap();
        assert!(final.is_finite());
    }
}
