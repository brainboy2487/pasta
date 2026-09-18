# MAGE API Reference

## C API

### TAP (The Answer Parser)

#### tap_parse_step

```c
int tap_parse_step(tap_context_t* ctx,
                   const char* input_feature,
                   uint32_t* next_state,
                   tap_metrics_t* metrics);
```

Executes one bounded parse step with strict 42-operation budget. Returns the next state determined by the stochastic transition model and populates metrics with operational statistics including operations used, shortlist size, early commit status, and confidence delta.

**Parameters:**
- ctx: TAP context containing current state and tensor metadata
- input_feature: Input token or feature string for classification
- next_state: Output pointer for determined next state index
- metrics: Output structure for parsing metrics

**Returns:** TAP_SUCCESS on success, error code otherwise

**Errors:**
- TAP_ERROR_INVALID_ARGS: Null pointer arguments
- TAP_ERROR_UNKNOWN_FEATURE: Feature not found in MPHF vocabulary
- TAP_ERROR_BUDGET_EXCEEDED: Operation budget violation (should never occur)

#### tap_context_create

```c
tap_context_t* tap_context_create(const tap_config_t* config);
```

Creates and initializes a TAP context from configuration. Allocates internal buffers, loads tensor metadata, and initializes the deterministic RNG with the specified seed.

**Parameters:**
- config: Configuration structure containing shortlist size, confidence threshold, and early commit settings

**Returns:** Pointer to allocated context, or NULL on allocation failure

#### tap_context_destroy

```c
void tap_context_destroy(tap_context_t* ctx);
```

Destroys TAP context and releases all associated resources.

**Parameters:**
- ctx: Context to destroy

### Contract (Lazy Block Contraction)

#### contract_lazy_block

```c
int contract_lazy_block(const mage_slice_t* slices,
                        const uint32_t* shortlist,
                        size_t shortlist_count,
                        size_t row_start,
                        size_t col_start,
                        size_t block_size,
                        double* output);
```

Executes lazy block contraction over shortlisted tensor slices. Generates only the requested B×B block rather than the full n×n matrix, using tier-appropriate generators for each slice.

**Parameters:**
- slices: Array of slice descriptors with tier, data pointers, and weights
- shortlist: Indices of slices to include in contraction
- shortlist_count: Number of slices in shortlist
- row_start: Starting row index for block
- col_start: Starting column index for block
- block_size: Dimension B of the square block
- output: Pre-allocated output buffer of size B×B doubles

**Returns:** CONTRACT_SUCCESS on success, error code otherwise

**Errors:**
- CONTRACT_ERROR_INVALID_ARGS: Null pointer arguments
- CONTRACT_ERROR_ALLOCATION_FAILED: Temporary buffer allocation failed
- CONTRACT_ERROR_INVALID_TIER: Unknown tier type encountered

## Python API

### MAGEContext

```python
class MAGEContext:
    def __init__(self, config=None)
    def parse_step(self, feature)
    def vectorize_text(self, text)
```

High-level Python interface to MAGE providing context management and text processing capabilities.

### Convenience Functions

#### vectorize_text

```python
def vectorize_text(text, rp_dim=128)
```

Convenience function for text vectorization. Processes input text through the forward feed pipeline and returns the compressed Random Projection embedding.

**Parameters:**
- text: Input text string to vectorize
- rp_dim: Target dimension for Random Projection (default 128)

**Returns:** numpy array of shape (rp_dim,) containing the normalized embedding vector

#### retrieve_similar

```python
def retrieve_similar(query_vector, corpus_vectors, k=5)
```

Retrieves k most similar vectors via cosine similarity using brute-force O(dN) search.

**Parameters:**
- query_vector: Query embedding vector
- corpus_vectors: Matrix of corpus vectors (N × d)
- k: Number of nearest neighbors to retrieve

**Returns:** Array of k indices and corresponding similarity scores

Generated: 2026-01-23 15:32:35

### Random Projection CLI flags

You can tune runtime Random Projection parameters via the CLI when invoking `mage_cli`.

- `--rp-k=<K>` : Set the number of RP projections used by TAP shortlist (overrides default `8`).
- `--rp-seed=<seed>` : Set the RP hashing seed (hex or decimal). Example: `--rp-seed=0x12345678`.

These flags set environment variables read by the runtime (`MAGE_RP_K`, `MAGE_RP_SEED`) and affect TAP contexts created with default configuration.

Persistent overrides

You can make RP settings persist across runs by using the CLI flags; the CLI writes a small `config/mage_runtime_overrides.json` file with the supplied values. TAP reads this file at context-creation time and applies `rp_K` and `hash_seed` if present. Example file contents:

```
{
    "tap": { "rp_K": 8 },
    "random_projection": { "hash_seed": 305419896 }
}
```
