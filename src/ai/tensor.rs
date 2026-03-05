// src/ai/tensor.rs
//! Tensor utilities backed by `ndarray` for performant numeric operations
//!
//! This module provides a `Tensor` type that uses the `ndarray` crate for
//! storage and vectorized operations. It integrates a small autograd-style
//! backward graph (reverse-mode) similar to `autograd.rs` but leverages
//! `ndarray` for efficient elementwise and linear-algebra kernels.
//!
//! Design goals:
//! - Use `ndarray::ArrayD<f64>` as the primary storage for data and gradients.
//! - Keep the autograd graph lightweight and Rust-native (Rc<RefCell<...>>).
//! - Provide common ops: add, sub, mul, div, neg, sum, mean, relu, powf, matmul.
//! - Use `ndarray`'s broadcasting and BLAS-backed dot when available for speed.
//!
//! Notes:
//! - This file assumes the `ndarray` crate is available in your Cargo.toml.
//!   Add `ndarray = "0.15"` (or a compatible version) and optionally `ndarray-linalg`
//!   if you want BLAS/LAPACK acceleration for large matrix multiplies.
//! - The autograd implementation is intentionally simple and educational; it is
//!   not a full-featured engine. For production workloads consider using `autograd`,
//!   `tch`, or `ndarray` + `autodiff` crates.

use std::cell::RefCell;
use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::rc::Rc;

use ndarray::{Array, ArrayD, ArrayViewD, Axis, IxDyn, IxDynImpl};
use ndarray::linalg::general_mat_mul;

type TensorRef = Rc<RefCell<TensorNode>>;

/// Public Tensor handle.
#[derive(Clone)]
pub struct Tensor {
    pub node: TensorRef,
}

impl fmt::Debug for Tensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.node.borrow();
        write!(
            f,
            "Tensor(shape={:?}, requires_grad={}, dtype=f64)",
            n.data.shape(),
            n.requires_grad
        )
    }
}

/// Internal node storing data, grad, and autograd metadata.
pub struct TensorNode {
    /// Data stored as dynamic ndarray.
    pub data: ArrayD<f64>,
    /// Gradient (same shape as data) if requires_grad.
    pub grad: Option<ArrayD<f64>>,
    /// Whether to track gradients.
    pub requires_grad: bool,
    /// Backward function to propagate gradients to parents.
    pub backward: Option<Box<dyn Fn(&TensorRef)>>,
    /// Parent references to keep them alive.
    pub parents: Vec<TensorRef>,
    /// Optional name for debugging.
    pub name: Option<String>,
}

impl TensorNode {
    fn new(data: ArrayD<f64>, requires_grad: bool) -> Self {
        let grad = if requires_grad {
            Some(ArrayD::zeros(data.raw_dim()))
        } else {
            None
        };
        Self {
            data,
            grad,
            requires_grad,
            backward: None,
            parents: Vec::new(),
            name: None,
        }
    }
}

// -----------------------------
// Constructors and helpers
// -----------------------------

impl Tensor {
    /// Create a tensor from an `ndarray::ArrayD<f64>`.
    pub fn from_array(data: ArrayD<f64>, requires_grad: bool) -> Self {
        Tensor {
            node: Rc::new(RefCell::new(TensorNode::new(data, requires_grad))),
        }
    }

    /// Create a scalar tensor.
    pub fn scalar(v: f64) -> Self {
        let arr = Array::from_elem(IxDyn(&[]), v).into_dyn();
        Tensor::from_array(arr, false)
    }

    /// Create a scalar that requires grad.
    pub fn scalar_requires_grad(v: f64) -> Self {
        let arr = Array::from_elem(IxDyn(&[]), v).into_dyn();
        Tensor::from_array(arr, true)
    }

    /// Create a tensor from Vec and shape.
    pub fn from_vec(data: Vec<f64>, shape: &[usize], requires_grad: bool) -> Self {
        let arr = Array::from_shape_vec(IxDyn(shape), data).expect("shape mismatch");
        Tensor::from_array(arr, requires_grad)
    }

    /// Create zeros tensor with shape.
    pub fn zeros(shape: &[usize], requires_grad: bool) -> Self {
        let arr = Array::zeros(IxDyn(shape));
        Tensor::from_array(arr, requires_grad)
    }

    /// Return shape as Vec<usize>.
    pub fn shape(&self) -> Vec<usize> {
        self.node.borrow().data.shape().to_vec()
    }

    /// Return a copy of the data as Vec<f64>.
    pub fn data(&self) -> Vec<f64> {
        self.node.borrow().data.iter().cloned().collect()
    }

    /// Ensure grad buffer exists and zero it.
    pub fn zero_grad(&self) {
        let mut n = self.node.borrow_mut();
        if n.requires_grad {
            if let Some(g) = &mut n.grad {
                g.fill(0.0);
            } else {
                n.grad = Some(ArrayD::zeros(n.data.raw_dim()));
            }
        }
    }

    /// Mark requires_grad true and allocate grad buffer.
    pub fn require_grad(&self) {
        let mut n = self.node.borrow_mut();
        if !n.requires_grad {
            n.requires_grad = true;
            n.grad = Some(ArrayD::zeros(n.data.raw_dim()));
        }
    }

    /// Sum all elements to a scalar tensor.
    pub fn sum(&self) -> Tensor {
        let n = self.node.borrow();
        let s = n.data.sum();
        let requires = n.requires_grad;
        drop(n);
        let out = Tensor::scalar_requires_grad(s);
        if requires {
            let a_ref = self.node.clone();
            out.node.borrow_mut().parents = vec![a_ref.clone()];
            let backward = Box::new(move |out_ref: &TensorRef| {
                let out_grad = out_ref.borrow().grad.as_ref().unwrap().clone();
                // out is scalar so out_grad is scalar
                let g = out_grad.into_scalar().unwrap_or(1.0);
                let mut a_mut = a_ref.borrow_mut();
                if let Some(ag) = &mut a_mut.grad {
                    // dL/dx += g * 1 for each element
                    ag += &ArrayD::from_elem(ag.raw_dim(), g);
                }
            });
            out.node.borrow_mut().backward = Some(backward);
        }
        out
    }

    /// Mean of elements to scalar.
    pub fn mean(&self) -> Tensor {
        let n = self.node.borrow();
        let s = n.data.mean().unwrap_or(0.0);
        let requires = n.requires_grad;
        let len = n.data.len() as f64;
        drop(n);
        let out = Tensor::scalar_requires_grad(s);
        if requires {
            let a_ref = self.node.clone();
            out.node.borrow_mut().parents = vec![a_ref.clone()];
            let inv_len = 1.0 / len;
            let backward = Box::new(move |out_ref: &TensorRef| {
                let out_grad = out_ref.borrow().grad.as_ref().unwrap().clone();
                let g = out_grad.into_scalar().unwrap_or(1.0);
                let mut a_mut = a_ref.borrow_mut();
                if let Some(ag) = &mut a_mut.grad {
                    ag += &ArrayD::from_elem(ag.raw_dim(), g * inv_len);
                }
            });
            out.node.borrow_mut().backward = Some(backward);
        }
        out
    }

    /// Elementwise ReLU
    pub fn relu(&self) -> Tensor {
        let a = self.node.borrow();
        let out_arr = a.data.mapv(|x| if x > 0.0 { x } else { 0.0 });
        let requires = a.requires_grad;
        drop(a);
        let out = Tensor::from_array(out_arr, requires);
        if requires {
            let a_ref = self.node.clone();
            out.node.borrow_mut().parents = vec![a_ref.clone()];
            let backward = Box::new(move |out_ref: &TensorRef| {
                let out_grad = out_ref.borrow().grad.as_ref().unwrap().clone();
                let a_data = a_ref.borrow().data.clone();
                let mut a_mut = a_ref.borrow_mut();
                if let Some(ag) = &mut a_mut.grad {
                    // ag += out_grad * (a_data > 0)
                    let mask = a_data.mapv(|x| if x > 0.0 { 1.0 } else { 0.0 });
                    *ag += &(&out_grad * &mask);
                }
            });
            out.node.borrow_mut().backward = Some(backward);
        }
        out
    }

    /// Elementwise power x^p where p is scalar.
    pub fn powf(&self, p: f64) -> Tensor {
        let a = self.node.borrow();
        let out_arr = a.data.mapv(|x| x.powf(p));
        let requires = a.requires_grad;
        drop(a);
        let out = Tensor::from_array(out_arr, requires);
        if requires {
            let a_ref = self.node.clone();
            out.node.borrow_mut().parents = vec![a_ref.clone()];
            let backward = Box::new(move |out_ref: &TensorRef| {
                let out_grad = out_ref.borrow().grad.as_ref().unwrap().clone();
                let a_data = a_ref.borrow().data.clone();
                let mut a_mut = a_ref.borrow_mut();
                if let Some(ag) = &mut a_mut.grad {
                    // derivative p * x^(p-1)
                    let deriv = a_data.mapv(|x| {
                        if x == 0.0 && p < 1.0 {
                            0.0
                        } else {
                            p * x.powf(p - 1.0)
                        }
                    });
                    *ag += &(&out_grad * &deriv);
                }
            });
            out.node.borrow_mut().backward = Some(backward);
        }
        out
    }

    /// Matrix multiplication for 2D tensors using `ndarray`'s dot.
    ///
    /// Expects both tensors to be 2D with compatible inner dims.
    pub fn matmul(&self, other: &Tensor) -> Tensor {
        let a = self.node.borrow();
        let b = other.node.borrow();
        assert!(a.data.ndim() == 2 && b.data.ndim() == 2, "matmul requires 2D tensors");
        let a_shape = a.data.shape().to_vec();
        let b_shape = b.data.shape().to_vec();
        let m = a_shape[0];
        let k = a_shape[1];
        let k2 = b_shape[0];
        let n = b_shape[1];
        assert!(k == k2, "matmul inner dims must match");
        // Use general_mat_mul for potential BLAS acceleration if enabled in ndarray build
        let mut out = Array::zeros(IxDyn(&[m, n]));
        // general_mat_mul(alpha, &a, &b, beta, &mut out) computes out = alpha * a.dot(b) + beta * out
        general_mat_mul(1.0, &a.data.view().into_dimensionality::<ndarray::Ix2>().unwrap(), &b.data.view().into_dimensionality::<ndarray::Ix2>().unwrap(), 0.0, &mut out.view_mut().into_dimensionality::<ndarray::Ix2>().unwrap());
        let requires = a.requires_grad || b.requires_grad;
        drop(a);
        drop(b);
        let out_t = Tensor::from_array(out, requires);
        if requires {
            let a_ref = self.node.clone();
            let b_ref = other.node.clone();
            out_t.node.borrow_mut().parents = vec![a_ref.clone(), b_ref.clone()];
            let backward = Box::new(move |out_ref: &TensorRef| {
                let out_grad = out_ref.borrow().grad.as_ref().unwrap().clone(); // m x n
                // dA += out_grad.dot(B.T)
                if a_ref.borrow().requires_grad {
                    let mut a_mut = a_ref.borrow_mut();
                    if let Some(ag) = &mut a_mut.grad {
                        // ag += out_grad.dot(B.T)
                        let outg2 = out_grad.view().into_dimensionality::<ndarray::Ix2>().unwrap();
                        let b2 = b_ref.borrow().data.view().into_dimensionality::<ndarray::Ix2>().unwrap();
                        let mut tmp = Array::zeros(ag.raw_dim());
                        general_mat_mul(1.0, &outg2, &b2.t(), 0.0, &mut tmp.view_mut().into_dimensionality::<ndarray::Ix2>().unwrap());
                        *ag += &tmp;
                    }
                }
                // dB += A.T.dot(out_grad)
                if b_ref.borrow().requires_grad {
                    let mut b_mut = b_ref.borrow_mut();
                    if let Some(bg) = &mut b_mut.grad {
                        let a2 = a_ref.borrow().data.view().into_dimensionality::<ndarray::Ix2>().unwrap();
                        let outg2 = out_grad.view().into_dimensionality::<ndarray::Ix2>().unwrap();
                        let mut tmp = Array::zeros(bg.raw_dim());
                        general_mat_mul(1.0, &a2.t(), &outg2, 0.0, &mut tmp.view_mut().into_dimensionality::<ndarray::Ix2>().unwrap());
                        *bg += &tmp;
                    }
                }
            });
            out_t.node.borrow_mut().backward = Some(backward);
        }
        out_t
    }

    /// Backpropagate from this tensor (if scalar, seeds grad=1).
    pub fn backward(&self) {
        {
            let mut n = self.node.borrow_mut();
            if n.grad.is_none() {
                n.grad = Some(ArrayD::zeros(n.data.raw_dim()));
            }
            // If scalar, set grad to 1
            if n.data.len() == 1 {
                if let Some(g) = &mut n.grad {
                    g.fill(0.0);
                    g.as_slice_mut().map(|s| s[0] = 1.0);
                }
            } else {
                // If non-scalar and grad is zero, set ones as default
                if let Some(g) = &mut n.grad {
                    if g.iter().all(|&x| x == 0.0) {
                        g.fill(1.0);
                    }
                }
            }
        }

        // Topological sort (post-order)
        let mut topo: Vec<TensorRef> = Vec::new();
        let mut visited: std::collections::HashSet<usize> = std::collections::HashSet::new();
        fn dfs(n: &TensorRef, topo: &mut Vec<TensorRef>, visited: &mut std::collections::HashSet<usize>) {
            let id = Rc::as_ptr(n) as usize;
            if visited.contains(&id) {
                return;
            }
            visited.insert(id);
            let parents = n.borrow().parents.clone();
            for p in parents {
                dfs(&p, topo, visited);
            }
            topo.push(n.clone());
        }
        dfs(&self.node, &mut topo, &mut visited);

        // Traverse reverse (from outputs to inputs)
        for node_ref in topo.into_iter().rev() {
            if let Some(b) = node_ref.borrow().backward.as_ref() {
                b(&node_ref);
            }
        }
    }
}

// -----------------------------
// Operator overloads using ndarray broadcasting
// -----------------------------

impl Add for &Tensor {
    type Output