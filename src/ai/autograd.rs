// src/ai/autograd.rs
//! Minimal autograd engine for PASTA AI utilities
//!
//! This module implements a small, self-contained automatic differentiation
//! system suitable for scalar and small tensor computations used in examples,
//! tests, and simple model prototypes. It is intentionally lightweight and
//! dependency-free: no ndarray, no external crates.
//!
//! Features:
//! - `Tensor` is a reference-counted, heap-backed n-dimensional array (row-major).
//! - Each `Tensor` may require gradients; operations build a backward graph.
//! - Supports basic ops: `add`, `sub`, `mul`, `div`, `neg`, `sum`, `mean`, `relu`,
//!   elementwise `powf`, and 2D `matmul`.
//! - Backpropagation via `backward()` computes gradients for all `requires_grad`
//!   leaves using reverse-mode autodiff.
//!
//! Limitations:
//! - No broadcasting beyond simple shape-equality and scalar broadcasting.
//! - No advanced memory optimizations; gradients are accumulated in-place.
//! - Not intended for production ML workloads — it's a pedagogical engine.

use std::cell::{RefCell, RefMut};
use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::rc::Rc;

/// Shared pointer to the inner tensor node.
pub type TensorRef = Rc<RefCell<TensorNode>>;

/// A Tensor is a handle to a TensorNode.
#[derive(Clone)]
pub struct Tensor {
    pub node: TensorRef,
}

impl fmt::Debug for Tensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.node.borrow();
        write!(
            f,
            "Tensor(shape={:?}, requires_grad={}, data_len={})",
            n.shape,
            n.requires_grad,
            n.data.len()
        )
    }
}

/// Internal representation of a tensor and its autograd metadata.
pub struct TensorNode {
    /// Flat row-major data.
    pub data: Vec<f64>,
    /// Shape (e.g., [2,3] for a 2x3 matrix). Empty vec for scalar.
    pub shape: Vec<usize>,
    /// Gradient accumulated for this tensor (same length as data) if requires_grad.
    pub grad: Option<Vec<f64>>,
    /// Whether to track gradients for this tensor.
    pub requires_grad: bool,
    /// Backward function to propagate gradients to parents.
    pub backward: Option<Box<dyn Fn(&TensorRef)>>,
    /// References to parent tensors (to keep them alive).
    pub parents: Vec<TensorRef>,
    /// Optional name for debugging.
    pub name: Option<String>,
}

impl TensorNode {
    fn new(data: Vec<f64>, shape: Vec<usize>, requires_grad: bool) -> Self {
        let grad = if requires_grad {
            Some(vec![0.0; data.len()])
        } else {
            None
        };
        Self {
            data,
            shape,
            grad,
            requires_grad,
            backward: None,
            parents: Vec::new(),
            name: None,
        }
    }

    fn numel(&self) -> usize {
        self.data.len()
    }

    fn is_scalar(&self) -> bool {
        self.shape.is_empty() || self.numel() == 1
    }
}

// -----------------------------
// Constructors and helpers
// -----------------------------

impl Tensor {
    /// Create a scalar tensor from a single f64.
    pub fn scalar(v: f64) -> Self {
        let node = TensorNode::new(vec![v], vec![], false);
        Tensor {
            node: Rc::new(RefCell::new(node)),
        }
    }

    /// Create a scalar that requires gradient.
    pub fn scalar_requires_grad(v: f64) -> Self {
        let mut node = TensorNode::new(vec![v], vec![], true);
        node.name = Some("scalar".into());
        Tensor {
            node: Rc::new(RefCell::new(node)),
        }
    }

    /// Create a tensor from Vec<f64> with given shape. `shape` product must equal data.len().
    pub fn from_vec(data: Vec<f64>, shape: Vec<usize>, requires_grad: bool) -> Self {
        let expected: usize = shape.iter().product();
        assert!(
            expected == data.len(),
            "shape product {} != data len {}",
            expected,
            data.len()
        );
        Tensor {
            node: Rc::new(RefCell::new(TensorNode::new(data, shape, requires_grad))),
        }
    }

    /// Create a zeros tensor with shape.
    pub fn zeros(shape: Vec<usize>, requires_grad: bool) -> Self {
        let len: usize = shape.iter().product();
        Tensor {
            node: Rc::new(RefCell::new(TensorNode::new(vec![0.0; len], shape, requires_grad))),
        }
    }

    /// Return shape.
    pub fn shape(&self) -> Vec<usize> {
        self.node.borrow().shape.clone()
    }

    /// Return data as slice.
    pub fn data(&self) -> Vec<f64> {
        self.node.borrow().data.clone()
    }

    /// Set a human-readable name for debugging.
    pub fn set_name(&self, name: impl Into<String>) {
        self.node.borrow_mut().name = Some(name.into());
    }

    /// Zero the gradients for this tensor and its descendants.
    pub fn zero_grad(&self) {
        let mut n = self.node.borrow_mut();
        if let Some(g) = &mut n.grad {
            for x in g.iter_mut() {
                *x = 0.0;
            }
        }
    }

    /// Mark this tensor as requiring gradients (allocates grad buffer).
    pub fn require_grad(&self) {
        let mut n = self.node.borrow_mut();
        if !n.requires_grad {
            n.requires_grad = true;
            n.grad = Some(vec![0.0; n.numel()]);
        }
    }

    /// Convenience: create a tensor from a single-element Vec and shape [] (scalar).
    pub fn from_f64(v: f64, requires_grad: bool) -> Self {
        let mut node = TensorNode::new(vec![v], vec![], requires_grad);
        if requires_grad {
            node.name = Some("scalar".into());
        }
        Tensor {
            node: Rc::new(RefCell::new(node)),
        }
    }

    /// Sum all elements into a scalar tensor.
    pub fn sum(&self) -> Tensor {
        let n = self.node.borrow();
        let s: f64 = n.data.iter().sum();
        drop(n);
        let out = Tensor::from_f64(s, self.node.borrow().requires_grad);
        if out.node.borrow().requires_grad {
            let a = self.node.clone();
            let b = out.node.clone();
            // backward: dL/dx_i += dL/dout * 1
            out.node.borrow_mut().parents = vec![a.clone()];
            let backward = Box::new(move |out_ref: &TensorRef| {
                let out_grad = out_ref.borrow().grad.as_ref().unwrap()[0];
                let mut a_mut = a.borrow_mut();
                if let Some(ag) = &mut a_mut.grad {
                    for v in ag.iter_mut() {
                        *v += out_grad;
                    }
                }
            });
            out.node.borrow_mut().backward = Some(backward);
        }
        out
    }

    /// Mean of elements into a scalar tensor.
    pub fn mean(&self) -> Tensor {
        let n = self.node.borrow();
        let s: f64 = n.data.iter().sum();
        let len = n.data.len() as f64;
        drop(n);
        let out = Tensor::from_f64(s / len, self.node.borrow().requires_grad);
        if out.node.borrow().requires_grad {
            let a = self.node.clone();
            let b = out.node.clone();
            let inv_len = 1.0 / (a.borrow().numel() as f64);
            out.node.borrow_mut().parents = vec![a.clone()];
            let backward = Box::new(move |out_ref: &TensorRef| {
                let out_grad = out_ref.borrow().grad.as_ref().unwrap()[0];
                let mut a_mut = a.borrow_mut();
                if let Some(ag) = &mut a_mut.grad {
                    for v in ag.iter_mut() {
                        *v += out_grad * inv_len;
                    }
                }
            });
            out.node.borrow_mut().backward = Some(backward);
        }
        out
    }

    /// Elementwise ReLU
    pub fn relu(&self) -> Tensor {
        let a = self.node.borrow();
        let mut out_data = Vec::with_capacity(a.data.len());
        for &x in &a.data {
            out_data.push(if x > 0.0 { x } else { 0.0 });
        }
        let requires = a.requires_grad;
        drop(a);
        let out = Tensor::from_vec(out_data, self.shape(), requires);
        if requires {
            let a_ref = self.node.clone();
            out.node.borrow_mut().parents = vec![a_ref.clone()];
            let backward = Box::new(move |out_ref: &TensorRef| {
                let out_grad = out_ref.borrow().grad.as_ref().unwrap().clone();
                let a_data = a_ref.borrow().data.clone();
                let mut a_mut = a_ref.borrow_mut();
                if let Some(ag) = &mut a_mut.grad {
                    for i in 0..ag.len() {
                        let grad_contrib = if a_data[i] > 0.0 { out_grad[i] } else { 0.0 };
                        ag[i] += grad_contrib;
                    }
                }
            });
            out.node.borrow_mut().backward = Some(backward);
        }
        out
    }

    /// Elementwise power (x^p) where p is scalar.
    pub fn powf(&self, p: f64) -> Tensor {
        let a = self.node.borrow();
        let mut out_data = Vec::with_capacity(a.data.len());
        for &x in &a.data {
            out_data.push(x.powf(p));
        }
        let requires = a.requires_grad;
        drop(a);
        let out = Tensor::from_vec(out_data, self.shape(), requires);
        if requires {
            let a_ref = self.node.clone();
            out.node.borrow_mut().parents = vec![a_ref.clone()];
            let backward = Box::new(move |out_ref: &TensorRef| {
                let out_grad = out_ref.borrow().grad.as_ref().unwrap().clone();
                let a_data = a_ref.borrow().data.clone();
                let mut a_mut = a_ref.borrow_mut();
                if let Some(ag) = &mut a_mut.grad {
                    for i in 0..ag.len() {
                        let deriv = if a_data[i] == 0.0 && p < 1.0 {
                            0.0
                        } else {
                            p * a_data[i].powf(p - 1.0)
                        };
                        ag[i] += out_grad[i] * deriv;
                    }
                }
            });
            out.node.borrow_mut().backward = Some(backward);
        }
        out
    }

    /// Matrix multiplication for 2D tensors: (m x k) @ (k x n) -> (m x n)
    pub fn matmul(&self, other: &Tensor) -> Tensor {
        let a = self.node.borrow();
        let b = other.node.borrow();
        assert!(a.shape.len() == 2 && b.shape.len() == 2, "matmul requires 2D tensors");
        let m = a.shape[0];
        let k = a.shape[1];
        let k2 = b.shape[0];
        let n = b.shape[1];
        assert!(k == k2, "matmul inner dimensions must match");
        let mut out = vec![0.0; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut s = 0.0;
                for t in 0..k {
                    s += a.data[i * k + t] * b.data[t * n + j];
                }
                out[i * n + j] = s;
            }
        }
        let requires = a.requires_grad || b.requires_grad;
        drop(a);
        drop(b);
        let out_t = Tensor::from_vec(out, vec![m, n], requires);
        if requires {
            let a_ref = self.node.clone();
            let b_ref = other.node.clone();
            out_t.node.borrow_mut().parents = vec![a_ref.clone(), b_ref.clone()];
            let backward = Box::new(move |out_ref: &TensorRef| {
                let out_grad = out_ref.borrow().grad.as_ref().unwrap().clone(); // m*n
                let a_data = a_ref.borrow().data.clone(); // m*k
                let b_data = b_ref.borrow().data.clone(); // k*n
                let m = a_ref.borrow().shape[0];
                let k = a_ref.borrow().shape[1];
                let n = b_ref.borrow().shape[1];

                // dA += dOut @ B^T
                if let Some(ag) = &mut a_ref.borrow_mut().grad {
                    for i in 0..m {
                        for t in 0..k {
                            let mut s = 0.0;
                            for j in 0..n {
                                s += out_grad[i * n + j] * b_data[t * n + j];
                            }
                            ag[i * k + t] += s;
                        }
                    }
                }

                // dB += A^T @ dOut
                if let Some(bg) = &mut b_ref.borrow_mut().grad {
                    for t in 0..k {
                        for j in 0..n {
                            let mut s = 0.0;
                            for i in 0..m {
                                s += a_data[i * k + t] * out_grad[i * n + j];
                            }
                            bg[t * n + j] += s;
                        }
                    }
                }
            });
            out_t.node.borrow_mut().backward = Some(backward);
        }
        out_t
    }

    /// Backpropagate from this tensor (assumed scalar) to all leaves that require gradients.
    ///
    /// If this tensor is not scalar, the caller must provide an initial gradient via `grad` argument.
    pub fn backward(&self) {
        // Seed gradient: if scalar, set grad = 1.0
        {
            let mut n = self.node.borrow_mut();
            if n.grad.is_none() {
                n.grad = Some(vec![0.0; n.numel()]);
            }
            // If scalar, set first element to 1.0
            if n.numel() == 1 {
                n.grad.as_mut().unwrap()[0] = 1.0;
            } else {
                // For non-scalar, user should have set grad externally; if not, set all ones.
                for v in n.grad.as_mut().unwrap().iter_mut() {
                    *v = 1.0;
                }
            }
        }

        // Post-order traversal: collect nodes
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

        // Traverse in reverse (from outputs to inputs)
        for node_ref in topo.into_iter().rev() {
            let maybe_backward = node_ref.borrow().backward.is_some();
            if maybe_backward {
                // Call backward closure
                let b = node_ref.borrow().backward.as_ref().unwrap().as_ref();
                b(&node_ref);
            }
        }
    }
}

// -----------------------------
// Basic arithmetic operator overloads
// -----------------------------

impl Add for &Tensor {
    type Output = Tensor;
    fn add(self, other: &Tensor) -> Tensor {
        let a = self.node.borrow();
        let b = other.node.borrow();
        // support scalar broadcasting
        let out_shape = if a.shape == b.shape {
            a.shape.clone()
        } else if a.is_scalar() {
            b.shape.clone()
        } else if b.is_scalar() {
            a.shape.clone()
        } else {
            panic!("add: incompatible shapes {:?} and {:?}", a.shape, b.shape);
        };
        let len: usize = out_shape.iter().product();
        let mut out = vec![0.0; len];
        for i in 0..len {
            let av = if a.numel() == 1 { a.data[0] } else { a.data[i] };
            let bv = if b.numel() == 1 { b.data[0] } else { b.data[i] };
            out[i] = av + bv;
        }
        let requires = a.requires_grad || b.requires_grad;
        drop(a);
        drop(b);
        let out_t = Tensor::from_vec(out, out_shape, requires);
        if requires {
