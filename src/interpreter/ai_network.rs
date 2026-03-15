//! src/interpreter/ai_network.rs
//!
//! Bridges PASTA's AI module with tensor creation to enable AI network development.
//!
//! This module provides interpreter-level utilities for:
//! - Creating neural network layers as tensor networks
//! - Building computation graphs from tensor operations
//! - Exposing AI models for training and inference through the REPL
//!
//! Exposes builtins like:
//! - `ai.linear(in_dim, out_dim)` — create a linear layer
//! - `ai.mlp([layer_dims...])` — create a multi-layer perceptron
//! - `ai.forward(model, input)` — run forward pass
//! - `ai.loss.mse(pred, target)` — mean squared error loss

use anyhow::{anyhow, Result};
use crate::interpreter::environment::{Value, RuntimeTensor};

/// A simple representation of an AI model layer for the interpreter.
/// Stored as a Value to be passed around in the REPL.
#[derive(Clone, Debug)]
pub enum AILayer {
    /// Linear layer: (weights, bias, in_dim, out_dim)
    Linear {
        /// Flattened weight matrix of shape `[out_dim][in_dim]`.
        weights: Vec<f64>,
        /// Bias vector of length `out_dim`.
        bias: Vec<f64>,
        /// Number of input features.
        in_dim: usize,
        /// Number of output features.
        out_dim: usize,
    },
    /// Activation layer
    ReLU,
    /// Softmax for classification
    Softmax,
}

impl AILayer {
    /// Create a new linear layer with random initialization
    pub fn linear(in_dim: usize, out_dim: usize) -> Self {
        // Xavier-style initialization: scale by sqrt(1 / in_dim)
        let scale = (1.0 / in_dim as f64).sqrt() * 0.5;
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;
        
        let mut weights = Vec::with_capacity(out_dim * in_dim);
        let mut state = seed;
        for _ in 0..(out_dim * in_dim) {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let u = ((state >> 33) as f64) / (u32::MAX as f64);
            // Box-Muller transform for normal distribution
            let v = ((state >> 33) as f64) / (u32::MAX as f64);
            let r = (-2.0 * u.ln()).sqrt();
            let z = r * (2.0 * std::f64::consts::PI * v).cos() * scale;
            weights.push(z);
        }
        
        let bias = vec![0.0; out_dim];
        Self::Linear { weights, bias, in_dim, out_dim }
    }

    /// Forward pass through a linear layer
    pub fn linear_forward(&self, input: &[f64]) -> Result<Vec<f64>> {
        match self {
            AILayer::Linear { weights, bias, in_dim, out_dim } => {
                if input.len() != *in_dim {
                    return Err(anyhow!("Linear forward: input size {} != expected {}", input.len(), in_dim));
                }
                let mut output = bias.clone();
                for i in 0..*out_dim {
                    let mut sum = 0.0;
                    for j in 0..*in_dim {
                        sum += weights[i * in_dim + j] * input[j];
                    }
                    output[i] += sum;
                }
                Ok(output)
            }
            _ => Err(anyhow!("linear_forward: not a linear layer")),
        }
    }

    /// Apply ReLU activation: max(0, x)
    pub fn relu(input: &[f64]) -> Vec<f64> {
        input.iter().map(|&x| if x > 0.0 { x } else { 0.0 }).collect()
    }

    /// Apply softmax for classification
    pub fn softmax(logits: &[f64]) -> Vec<f64> {
        let max = logits.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = logits.iter().map(|&x| (x - max).exp()).collect();
        let sum_exp: f64 = exps.iter().sum();
        exps.iter().map(|e| e / sum_exp).collect()
    }
}

/// A simple feedforward neural network (MLP) for the interpreter
#[derive(Clone, Debug)]
pub struct NeuralNetwork {
    /// Ordered sequence of layers in the network.
    pub layers: Vec<AILayer>,
    /// Human-readable names for each layer (same length as `layers`).
    pub layer_names: Vec<String>,
}

impl NeuralNetwork {
    /// Create a new MLP with given layer dimensions
    /// e.g., [input_dim, hidden1, hidden2, output_dim]
    pub fn mlp(dims: &[usize]) -> Result<Self> {
        if dims.len() < 2 {
            return Err(anyhow!("MLP requires at least 2 dimensions (input, output)"));
        }
        
        let mut layers = Vec::new();
        let mut layer_names = Vec::new();
        
        for i in 0..(dims.len() - 1) {
            let in_d = dims[i];
            let out_d = dims[i + 1];
            layers.push(AILayer::linear(in_d, out_d));
            layer_names.push(format!("linear_{}", i));
            
            // Add ReLU activation after each layer except the last
            if i < dims.len() - 2 {
                layers.push(AILayer::ReLU);
                layer_names.push(format!("relu_{}", i));
            }
        }
        
        Ok(NeuralNetwork { layers, layer_names })
    }

    /// Forward pass through the network
    pub fn forward(&self, input: &[f64]) -> Result<Vec<f64>> {
        let mut x = input.to_vec();
        
        for layer in &self.layers {
            x = match layer {
                AILayer::Linear { .. } => layer.linear_forward(&x)?,
                AILayer::ReLU => AILayer::relu(&x),
                AILayer::Softmax => AILayer::softmax(&x),
            };
        }
        
        Ok(x)
    }

    /// Export network as a Value for storage in interpreter environment
    pub fn to_value(&self) -> Value {
        // Store as a descriptive string; could also use a custom heap value
        Value::String(format!(
            "NeuralNetwork with {} layers: {}",
            self.layers.len(),
            self.layer_names.join(" -> ")
        ))
    }
}

/// Create a tensor from network output
pub fn output_to_tensor(data: Vec<f64>) -> RuntimeTensor {
    RuntimeTensor::new(
        vec![data.len()],
        "float32".to_string(),
        data,
    )
}

/// Mean squared error loss between prediction and target
pub fn mse_loss(pred: &[f64], target: &[f64]) -> Result<f64> {
    if pred.len() != target.len() {
        return Err(anyhow!("MSE loss: prediction and target have different lengths"));
    }
    let mse = pred
        .iter()
        .zip(target.iter())
        .map(|(p, t)| (p - t).powi(2))
        .sum::<f64>() / pred.len() as f64;
    Ok(mse)
}

/// Cross-entropy loss for classification
pub fn cross_entropy_loss(logits: &[f64], target_class: usize) -> Result<f64> {
    if target_class >= logits.len() {
        return Err(anyhow!("Cross-entropy loss: target class {} out of range {}", target_class, logits.len()));
    }
    let probs = AILayer::softmax(logits);
    let loss = -probs[target_class].ln();
    Ok(loss)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_layer_forward() {
        let layer = AILayer::linear(2, 3);
        let input = vec![1.0, 2.0];
        let output = layer.linear_forward(&input);
        assert!(output.is_ok());
        let out = output.unwrap();
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn relu_activation() {
        let input = vec![-1.0, 0.0, 1.0, 2.0];
        let output = AILayer::relu(&input);
        assert_eq!(output, vec![0.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn softmax_sums_to_one() {
        let logits = vec![1.0, 2.0, 3.0];
        let probs = AILayer::softmax(&logits);
        let sum: f64 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn mlp_forward_pass() {
        let net = NeuralNetwork::mlp(&[2, 4, 3]).unwrap();
        let input = vec![1.0, 2.0];
        let output = net.forward(&input);
        assert!(output.is_ok());
        let out = output.unwrap();
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn mse_loss_calculation() {
        let pred = vec![1.0, 2.0, 3.0];
        let target = vec![1.1, 2.1, 2.9];
        let loss = mse_loss(&pred, &target).unwrap();
        assert!(loss > 0.0);
        assert!(loss < 0.1); // small loss, but not necessarily < 0.01
    }
}
