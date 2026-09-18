**RP API**

Overview
- Purpose: deterministic sparse random projection helpers used by Mage for
  reproducible hashing-based projections and feature hashing.
- File: `include/mage/rp.h`, implementation: `src/core/rp.c`.

Key functions
- `uint64_t mage_hash_u64(uint64_t x, uint64_t seed)`
  - Returns a 64-bit pseudorandom value derived from `(x, seed)`. Uses an
    internal splitmix64-style mixer. Deterministic and platform-independent
    for a given compiler/architecture (same algorithmic output).

- `int mage_hash_sign64(uint64_t key, uint64_t seed)`
  - Returns +1 or -1 deterministically for `(key,seed)`. Implemented by
    taking a low bit of `mage_hash_u64`.

- `size_t mage_rp_index(uint64_t key, size_t dim, uint64_t seed)`
  - Maps `(key,seed)` into an index in `[0, dim)`. Uses multiply-shift
    mapping: `index = floor(h * dim / 2^64)` where `h = mage_hash_u64(...)`.
  - This reduces modulo bias compared to `h % dim` and is preferred for
    non-power-of-two `dim`.

- `void mage_rp_project(uint64_t key, size_t dim, uint64_t seed, size_t *idx_out, int *sign_out)`
  - Convenience that fills `*idx_out` and `*sign_out` for one projection.
  - Sign is derived with an independent seed mix (XOR with constant) so
    index and sign are effectively independent.

- `void mage_rp_project_k(uint64_t key, size_t dim, uint64_t seed, size_t K, size_t *idx_out, int *sign_out, int unique)`
  - Produces `K` (index, sign) pairs for a single key.
  - If `unique` is non-zero, linear-probes on collisions to attempt uniqueness.
  - Uses a deterministic stride and splitmix64 variants to derive `K`
    independent hashes per key.

Behavior & recommendations
- Seeding: choose a stable 64-bit `seed` per run/version to make projections
  repeatable across process restarts. Mix different constants when deriving
  independent outputs (index vs sign) as implemented.
- For K-index sparse projections, `unique=1` makes outputs unique by linear
  probing; this is deterministic but can be slower for high `K` relative to
  `dim` — consider rehash-based schemes if performance is critical.
- For cross-platform bit-exact floating-point accumulation of many projected
  values, prefer defining a fixed reduction tree or using fixed-point
  accumulation; otherwise small differences in FP rounding/associativity may
  appear between compilers/architectures.

Testing & parity
- Golden generation: `tools/gen_rp_golden.c` (used in CI) produces
  `tests/golden/rp_golden.txt` by calling `mage_rp_project` so parity is exact.
- Unit tests added:
  - `tests/unit/test_rp.c` — determinism and sign distribution checks.
  - `tests/unit/test_rp_parity.c` — compares C outputs to generated golden.
  - `tests/unit/test_rp_k.c` — tests `K`-projection determinism and bounds.

Examples
- Single projection (C):

  size_t idx; int s;
  mage_rp_project(key, 1024, 0x12345678ULL, &idx, &s);

- K-projection (C):

  size_t idxs[4]; int signs[4];
  mage_rp_project_k(key, 1024, 0x12345678ULL, 4, idxs, signs, 1);

Makefile / CI
- `make tests/golden/rp_golden.txt` builds `tools/gen_rp_golden` and
  regenerates goldens. `make ci` runs tokenizer + RP unit + parity + K tests.

Notes
- The multiply-shift mapping uses 128-bit intermediate arithmetic (C `__uint128_t`)
  and requires compiler support (GCC/Clang). If porting to constrained
  toolchains without 128-bit support, implement unbiased mapping via
  rejection sampling or platform-specific intrinsics.
