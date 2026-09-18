# MAGE Architecture Documentation

## System Overview

The Markov Answer Generation Engine implements a compute-for-memory tradeoff that addresses the fundamental memory bandwidth bottleneck in dense tensor operations. The system achieves this through three key architectural components: the Tiered Stochastic Tensor Framework, The Answer Parser (TAP) with bounded operation semantics, and Lazy Block Contraction.

Generated: 2026-01-23 15:32:35

## Core Components

### The Answer Parser (TAP)

The Answer Parser enforces a strict 42-operation budget per parse step, guaranteeing predictable, bounded latency regardless of input complexity. This constraint is achieved through careful allocation of computational resources across five stages: Feature Encoding (8 operations), Context Predicates (12 operations), Shortlist Selection (10 operations), Micro-Contraction (8 operations), and Commit/Fallback (4 operations).

The Feature Encoding stage requires the use of a Minimal Perfect Hash Function (MPHF) to ensure O(1) worst-case lookup time. Standard hash tables would violate the bounded operation guarantee due to potential collision chains. The MPHF maps input features (tokens, grammar classes) to indices with no collisions for the fixed vocabulary.

The Shortlist Selection stage implements an O(K log S) heap-based algorithm to select the most informative tensor slices using a gain-cost heuristic. The score for each slice is computed as (weight × (1 + recency)) / tier_cost, prioritizing slices that provide maximum information gain per computational cost unit.

### Tiered Tensor Framework

The system organizes tensor slices into three tiers based on access patterns and computational tradeoffs:

The Hot tier maintains full dense storage for the highest-weight slices, providing minimal latency at approximately 0.66 nanoseconds per element. This tier is memory bandwidth limited and reserved for slices that are accessed frequently or have critical importance to the final result.

The Warm tier uses low-rank factorization (T ≈ U V^T) to achieve 30-500× memory reduction compared to Hot tier storage. Each element is reconstructed through O(r) multiply-add operations, resulting in approximately 7.1 nanoseconds per element. The critical optimization requirement is that the V matrix must be stored in transposed form to enable contiguous memory access during inner product computation, allowing for effective SIMD vectorization.

The Cold tier employs separable analytic functions where T[i,j] = f(i) × g(j), reducing 2D function evaluation to two 1D evaluations followed by an outer product. This approach provides over 1000× memory reduction and significantly improves upon naive 2D polynomial evaluation by precomputing 1D function values for the entire block.

### Lazy Block Contraction

Rather than materializing the full n × n transition matrix for all K tensor dimensions, the system generates only the requested B × B blocks for the shortlisted slices. The block size B is typically tuned to 64, ensuring that the working set fits within L1/L2 cache (32 KB for double precision).

The contraction accumulates weighted blocks from each shortlisted slice, with the tier-specific generator producing each block on demand. This lazy evaluation strategy transforms the problem from a memory-bound operation (reading 512+ MB) to a compute-bound operation with manageable memory footprint.

## Determinism Guarantees

MAGE enforces strict determinism through several engineering protocols. The system uses a fixed Random Number Generator seed (xorshift32) initialized at startup. Block processing follows a deterministic ordering, and parallel reduction operations use either pairwise reduction or a fixed tree-reduction order to ensure identical floating-point results across runs.

For the Cold tier, intermediate accumulations during fixed-point table construction use 64-bit precision accumulators to prevent rounding error accumulation, even when final values are stored with lower precision.

## Performance Characteristics

The memory-for-compute tradeoff is validated through empirical measurements. A moderately sized tensor with n=1024 states and K=64 slices would require reading approximately 67.1 million elements (512 MB) in a dense approach, with memory operation time of approximately 0.052 seconds dominating the theoretical compute time of 0.0048 seconds.

The hybrid memory model shifts this balance, allowing generation time to approximate read time for the reduced parameter sets. The resulting end-to-end latency is significantly lower than the dense approach while maintaining numerical accuracy within specified tolerances.

## Error Bounds and Accuracy

TAP provides an explicit accuracy guarantee through the bounded omission error. When slices are omitted from the shortlist, the per-row L1 error is bounded by the total mass of omitted slices (W_om). The system is configured to ensure W_om remains below an application-specific tolerance ε, typically on the order of 0.001 to 0.01.

The confidence-based early commit mechanism uses the delta metric (Δ = P[i, j_best] - P[i, j_second_best]) to determine when sufficient certainty has been achieved. When Δ exceeds the threshold τ, the system commits immediately using fewer than 42 operations, following the principles of Optimal Computing Budget Allocation.

## Scalability Considerations

For higher-dimensional tensors where K can exceed 32,768 slices, the heap-based selection algorithm's O(K log S) complexity becomes critical. Without this optimization, the naive O(K²) approach would violate the low-latency mandate as K increases.

Future scalability improvements include the implementation of approximate nearest neighbor indexing (such as HNSW) to replace the prototype's brute-force O(dN) retrieval, and the development of distributed contraction strategies for extremely large tensor spaces.

## File Formats and Interoperability

The system uses three canonical file formats for data interchange. The cleaned.jsonl format stores normalized document text with one JSON object per line containing text, title, and URL fields. The vectors.bin format is a binary file with a fixed header (two uint32 values for number of vectors and dimension) followed by float32 arrays in row-major order. The manifest.jsonl format provides the mapping between documents and vector indices, including metadata for reconstruction and retrieval.

These formats are designed for cross-platform stability and simple integration with external tools through Python FFI or direct file parsing.
