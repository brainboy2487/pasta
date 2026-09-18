Research

A single canonical document that consolidates every idea, math derivation, implementation detail, test artifact, and operational recommendation we developed for MAGE (Markov Answer Generation Engine). This file is intended to be the authoritative reference for engineers, researchers, and integrators building the 1D→2D→3D→4D→5D binaries, the TAP bounded parser, and the hybrid memory/compute slice generators.

---

Overview

Project name  
MAGE — Markov Answer Generation Engine

Core idea  
Shift cost from memory bandwidth to predictable, auditable compute by procedurally generating tensor slices on demand, using separable, low‑rank, analytic, or compressed representations. Combine this with a bounded‑operation parser (TAP) that enforces strict per‑step budgets and a deterministic sampling and reduction pipeline.

Goals
- Deterministic behavior across runs and threads.  
- Bounded latency per parse step via TAP (42 primitive ops budget).  
- Memory efficiency through on‑the‑fly slice generation and tiered storage.  
- Auditability: explicit math, error bounds, and reproducible kernels.  
- CPU-first design: run on commodity devices without GPUs.

High level levers
- Reduce effective slice count \(K\) via TAP shortlist and weight mass thresholds.  
- Replace stored \(n\times n\) slices with parameterized generators (separable, low‑rank, analytic).  
- Block/tile contraction to maximize cache reuse.  
- Fixed‑point and integer decode for cold slices to reduce bandwidth.

---

Architecture and Components

Core components
- Tensors and slices: base state size \(n\); extra axes sizes \(k,m,\dots\). Slices are \(n\times n\) matrices indexed by extra axes.  
- Weighting engine: mixture of exponential and power‑law kernels, optional attention scaling, clipping, and normalization.  
- Slice generators:
  - Hot: full stored slices (raw doubles).  
  - Warm: low‑rank factorization \(U^{(t)}V^{(t)\top}\) or separable factors \(h(i,t)g(j,t)\).  
  - Cold: analytic functions (Gaussian mixtures, Fourier/DCT basis, polynomial surfaces) or compressed encodings.  
- Contraction engine: block‑wise accumulation into effective \(n\times n\) matrix \(P\).  
- Normalization and sampling: per‑row normalization and CDF scan sampling with deterministic RNG.  
- TAP: bounded‑operation parser that selects slices under a cost budget using a gain/cost heuristic.  
- Promotion/demotion policy: dynamic tier movement based on weight × error and recency.

Data flow
1. Compute weights \(w_\alpha\) for extra axes.  
2. TAP selects a shortlist of slices to examine under budget.  
3. For each selected slice, call the appropriate generator to produce \(B\times B\) blocks and accumulate into the output block.  
4. After all blocks processed, perform deterministic reductions and row normalization.  
5. Sample next state or commit early if confidence threshold met.  
6. Optionally write back updates to slices with decay.

Minimal C ABI (canonical prototypes)
`c
typedef struct { unsigned int s; } rng32_t;
void rng32seed(rng32t *r, unsigned int seed);
double rng32uniform(rng32t *r);

void normalizerows(double *M, sizet n);
void blendmatrices(double out, const double mats, const double w, sizet n, size_t k);

void effectivetransition3d(double out, const double T3, const double *w, sizet n, sizet k);
void effectivetransition4d(double out, const double T4, const double wt, const double vc, sizet n, sizet k, size_t m);

int samplenext(const double P, sizet n, int i, rng32_t rng);
int tapparsestep(int nextout, const double features, const void tables, const double Pactive, size_t n, int budget);
`

---

Math Guide and Formulas

Notation
- n: number of states.  
- k, m, ...: sizes of extra axes.  
- K: product of extra axis sizes \(K=\proda ka\).  
- T[:,:,α]: slice indexed by multi‑index \(\boldsymbol{\alpha}\).  
- w_α: weight for slice \(\boldsymbol{\alpha}\).  
- P: effective \(n\times n\) transition matrix.

Contraction
3D contraction
\[
P = \sum{t=0}^{k-1} wt\,T[:,:,t]
\]

4D contraction
\[
P = \sum{t=0}^{k-1}\sum{c=0}^{m-1} wt\,vc\,T[:,:,t,c]
\]

dD contraction
\[
P = \sum{\boldsymbol{\alpha}} W{\boldsymbol{\alpha}}\,T[:,:, \boldsymbol{\alpha}]
\]

Elements touched
\[
\text{Elements} = n^2 \cdot K
\]

FLOPs for contraction
\[
\text{FLOPs}_{\text{contract}} \approx 2 \cdot n^2 \cdot K
\]

Memory bytes read
\[
\text{Bytes} \approx 8 \cdot n^2 \cdot K
\]

Time estimates
Let \(P\) be per‑core FLOPs/sec, \(C\) cores, efficiency \(\eta\), bandwidth \(B\) bytes/sec:
\[
T_{\text{compute}} \approx \frac{2 n^2 K}{C P \eta},\quad
T_{\text{mem}} \approx \frac{8 n^2 K}{B}
\]
Runtime ≈ \(\max(T{\text{compute}}, T{\text{mem}})\).

Normalization and sampling
Row normalization
For each row \(i\):
\[
si = \sum{j=0}^{n-1} P_{ij},\quad
P_{ij} \leftarrow \begin{cases}
P{ij}/si & s_i>0\\
1/n & s_i\le 0
\end{cases}
\]

Sampling
CDF scan per row \(i\):
\[
\text{acc}\leftarrow 0;\quad \text{for } j: \text{acc}\mathrel{+}=P_{ij};\; r\le\text{acc}\Rightarrow j
\]

Weighting kernels
Power law
\[
w^{\text{pow}}_t \propto (t+1)^{-\alpha},\quad
w^{\text{pow}}_t = \exp\big(-\alpha \log(t+1)\big)
\]

Exponential
\[
w^{\exp}t \propto \lambda^t,\quad wt = w_{t-1}\cdot\lambda
\]

Mixture
\[
\tilde wt = \gamma\,\tilde w^{\exp}t + (1-\gamma)\,\tilde w^{\text{pow}}_t
\]
With attention scores \(s_t\):
\[
wt \leftarrow \text{clip}\big(wt \cdot e^{\beta s_t}, \epsilon, 1-\epsilon\big)
\]
Normalize:
\[
wt \leftarrow \frac{wt}{\sumu wu}
\]

Error bounds for approximate contraction
If omitted slices have total weight mass \(W{\text{om}}\) and per‑slice rows are normalized, then per‑row \(L1\) error satisfies:
\[
\sumj |P{ij} - \hat P{ij}| \le W{\text{om}}
\]
Use this to set thresholds for shortlist size and tier demotion.

Low‑rank and separable approximations
Low‑rank factorization
\[
T[:,:,t] \approx U^{(t)} V^{(t)\top},\quad U^{(t)}\in\mathbb{R}^{n\times r},\; V^{(t)}\in\mathbb{R}^{n\times r}
\]
Storage per slice: \(2 n r\) floats. Per element cost: \(O(r)\) multiplies/adds.

Separable kernel
\[
T[i,j,t] = h(i,t)\cdot g(j,t)
\]
Storage per slice: \(2n\) floats. Per element cost: 1 multiply.

Basis expansion
\[
T[i,j,t] = \sum{b=0}^{B-1} cb(t)\,B_b[i,j]
\]
Per element cost: \(B\) mul‑adds.

TAP gating and cost model
- Primitive ops: compare, branch, lookup, small contraction, sample.  
- Budget: 42 primitive ops per parse step.  
- TAP selects slices maximizing information gain per cost \(gt/ct\) until budget exhausted.  
- Confidence metric for early commit:
\[
\Delta = P{i j^\ast} - P{i j^{(2)}}
\]
Commit if \(\Delta \ge \tau\).

---

Implementation Details and Code Artifacts

Single-file artifacts produced
- weightsnolibnoheaders.c: pure C weight generator with logapprox, expapprox, pow_approx, mixture kernels, fixed‑point table build/decode, no headers. Compile:
`bash
clang -O3 -std=c99 weightsnolibnoheaders.c -o weightsnolibnoheaders
`
- weightsnolibhardened.c: sanitizer‑friendly variant using standard headers for debugging. Compile with sanitizers:
`bash
clang -g -O0 -std=c99 weightsnolibhardened.c -o weightsnolibdebug -fsanitize=address,undefined -lm
`
- markov_3d.c: minimal 3D contraction test with deterministic slice builder, normalization, contraction, sampling. Compile:
`bash
clang -O3 -std=c99 markov3d.c -o markov3d
`
- 3dweighttest.c: consolidated, hardened, no‑headers single file that integrates weight kernels, fixed‑point safe build/decode, deterministic slice builder, contraction, normalization, sampling, and debug prints. Compile:
`bash
clang -O3 -std=c99 3dweighttest.c -o 3dweighttest
`

Key implementation patterns
- Block generation API (recommended stub)
`c
/ generate BxB block for slice t into accumulator P_block /
void generateblock(double *Pblock, unsigned long n, unsigned long i0, unsigned long j0,
                    unsigned long B, unsigned long t, const void *params_t);
`
- Cache-friendly blocking: choose \(B\) so \(8 B^2 \times\) (buffers) fits L1/L2. Example \(B=64\) yields 32 KB per double block.
- Deterministic reductions: fixed worker ordering or pairwise tree with fixed pairing to ensure reproducible sums.
- Promotion/demotion metadata per slice:
`c
struct slice_meta {
  double weight_estimate;
  double approx_error;
  unsigned int last_used;
  unsigned int tier; / 0 hot, 1 warm, 2 cold /
};
`

Compile and run notes
- Use -O3 -march=native -fno-math-errno -ffast-math for release builds where safe.  
- Use sanitizer builds during development to catch UB and memory errors.  
- For Termux or constrained environments, prefer the no‑headers files and the pure‑C approximations.

---

Quantization, Rounding, and Performance Tradeoffs

Options compared

| Option | Storage per element | Exact representability | Max absolute error | Practical effect |
|---|---:|---|---:|---|
| No rounding (double) | 8 bytes | Yes | 0 | Baseline |
| Binary quantize to \(2^{-q}\) stored as double | 8 bytes | Yes for rounded values | \(\le 2^{-(q+1)}\) | No storage gain unless re-encoded |
| Float (32-bit) | 4 bytes | No | \(\approx 2^{-24}\) relative | 2× bandwidth reduction; simple |
| Fixed-point int32 Q bits | 4 bytes | Yes for quantized values | \(\le 2^{-(Q+1)}\) | 2× bandwidth reduction; integer ops possible |
| Fixed-point int16 Q bits | 2 bytes | Yes for quantized values | \(\le 2^{-(Q+1)}\) | 4× bandwidth reduction; accuracy risk |

Practical recommendations
- If memory bound: convert to float first for a simple 2× bandwidth win.  
- If deterministic integer arithmetic desired: use fixed‑point int32 with chosen Q fractional bits. Choose Q from tolerance \(\varepsilon\) using:
\[
q \ge -1 - \log_2(\varepsilon)
\]
- If you need exact binary representability of quantized values: quantize to multiples of \(2^{-q}\) and store as integer or as double with exact multiples.
- Avoid decimal rounding alone: it does not reduce bandwidth or speed unless paired with a storage type change.

Minimal rounding that is "lossless" for storage
- No rounding is truly lossless. The minimal binary‑aligned rounding that produces exactly representable values is rounding to multiples of \(2^{-q}\). This is not lossless relative to original data but yields exact storage of the rounded values.

---

Testing, Benchmarks, and Validation

Tests included
- Unit tests: row normalization invariants, deterministic RNG outputs, fixed‑point build/decode roundtrip.  
- Integration tests: 3D contraction correctness, sampling chain sanity, TAP budget enforcement (skeleton).  
- Property tests: Monte Carlo stationary checks for ergodic matrices, energy retention checks for low‑rank approximations.

Microbench plan
- Measure ns/element for:
  - weightsexponentialiter (cheap).  
  - weights_powerlaw (approx via log+exp).  
  - weightsmixturepowerexp.  
  - contract_3d for varying \(n\) and \(K\).  
- Measure memory throughput and compute throughput to determine whether workload is memory‑bound or compute‑bound. Use the time estimates in the Math Guide to interpret results.

Example profiling targets
- Small device: \(n=1024\), \(k=16\), \(m=4\) ⇒ \(K=64\).  
- Measure:
  - Bytes read ≈ \(8 n^2 (K+1)\).  
  - FLOPs ≈ \(2 n^2 K\).  
  - Compare \(T{\text{mem}}\) and \(T{\text{compute}}\) to decide whether to increase compute (procedural generation) or reduce memory.

---

Next Steps and Roadmap

Immediate
- Integrate the no‑headers weight generator into the 3D contraction harness as the canonical weight source.  
- Implement the block generator API with three concrete generators: separable, low‑rank, analytic.  
- Add TAP skeleton with cost accounting and a greedy gain/cost selector.

Short term
- Run hybrid tier simulations with real token traces to tune promotion/demotion thresholds and \(r\) for low‑rank.  
- Implement fixed‑point decode paths for cold slices and measure end‑to‑end latency.

Medium term
- Build libmage.so and Python FFI examples.  
- Add SIMD acceleration for polynomial evaluations and block inner loops.  
- Prepare reproducible benchmarks and packaging for embedded targets.

Long term
- Release reproducible artifacts: precomputed basis matrices, sample slice parameter sets, and TAP trace datasets.  
- Optimize for embedded and mobile devices with integer kernels and quantized storage.

---

> Tip: Use the per‑slice weight mass and approximation residual as the single unified metric for tier decisions. Promote slices when weight × residual > threshold and demote when weight falls below a time‑decayed threshold.

---

I can now generate the block generator C stub and the microbenchmark harness that implements separable and low‑rank generators and measures ns/element for contraction and generator costs; reply with Generate to proceed.