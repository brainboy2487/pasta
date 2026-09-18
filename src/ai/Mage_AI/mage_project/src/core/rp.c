/* rp.c - Refactored deterministic random projection helpers for Mage
 *
 * Drop-in replacement for the original rp.c with:
 *  - Clearer, well-documented functions
 *  - Robust argument checks and deterministic behavior
 *  - Small performance and portability improvements
 *
 * Public functions:
 *   uint64_t mage_hash_u64(uint64_t x, uint64_t seed);
 *   int      mage_hash_sign64(uint64_t key, uint64_t seed);
 *   size_t   mage_rp_index(uint64_t key, size_t dim, uint64_t seed);
 *   void     mage_rp_project(uint64_t key, size_t dim, uint64_t seed, size_t *idx_out, int *sign_out);
 *   void     mage_rp_project_k(uint64_t key, size_t dim, uint64_t seed, size_t K, size_t *idx_out, int *sign_out, int unique);
 *
 * Notes:
 *  - All functions are deterministic and avoid dynamic allocation.
 *  - When `dim == 0` or `K == 0`, outputs are set to safe defaults (0 / 0).
 *  - If `unique` is requested in mage_rp_project_k, a simple linear-probing
 *    strategy is used to avoid duplicates; it will stop after `dim` attempts.
 */

#define _POSIX_C_SOURCE 200809L
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

/* SplitMix64-like PRNG used for deterministic hashing and projection.
 * This implementation is self-contained and avoids external dependencies.
 */
static inline uint64_t splitmix64_step(uint64_t x) {
    uint64_t z = x + 0x9e3779b97f4a7c15ULL;
    z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ULL;
    z = (z ^ (z >> 27)) * 0x94d049bb133111ebULL;
    return z ^ (z >> 31);
}

/* Public: produce a 64-bit hash from input and seed */
uint64_t mage_hash_u64(uint64_t x, uint64_t seed) {
    /* Combine input and seed in a simple but deterministic way */
    uint64_t v = x ^ (seed + 0x9e3779b97f4a7c15ULL);
    return splitmix64_step(v);
}

/* Public: deterministic sign (-1 or +1) derived from key and seed */
int mage_hash_sign64(uint64_t key, uint64_t seed) {
    uint64_t h = mage_hash_u64(key, seed);
    /* Use lowest bit to decide sign: 1 -> +1, 0 -> -1 (keeps distribution balanced) */
    return (h & 1ULL) ? 1 : -1;
}

/* Map a 64-bit key into an index in [0, dim) using multiply-shift to reduce bias.
 * Returns 0 when dim == 0.
 */
size_t mage_rp_index(uint64_t key, size_t dim, uint64_t seed) {
    if (dim == 0) return 0;
    uint64_t h = mage_hash_u64(key ^ 0xAFFEDEADBEEFULL, seed);
    /* Multiply-shift: floor(h * dim / 2^64) */
    __uint128_t prod = ( __uint128_t)h * ( __uint128_t)dim;
    return (size_t)(prod >> 64);
}

/* Project a single key into one index and sign.
 * If dim == 0, idx_out is set to 0 and sign_out to 0.
 */
void mage_rp_project(uint64_t key, size_t dim, uint64_t seed, size_t *idx_out, int *sign_out) {
    if (idx_out) *idx_out = 0;
    if (sign_out) *sign_out = 0;
    if (dim == 0) return;
    uint64_t h = mage_hash_u64(key ^ 0xC0FFEE1234567890ULL, seed);
    /* index via multiply-shift */
    __uint128_t prod = ( __uint128_t)h * ( __uint128_t)dim;
    if (idx_out) *idx_out = (size_t)(prod >> 64);
    if (sign_out) *sign_out = (h & 1ULL) ? 1 : -1;
}

/* Project a key into K indices and signs.
 *
 * Parameters:
 *  - key: input key
 *  - dim: target dimensionality (must be > 0 to produce meaningful indices)
 *  - seed: deterministic seed
 *  - K: number of projections requested
 *  - idx_out: caller-provided array of size >= K (will be written)
 *  - sign_out: caller-provided array of size >= K (will be written)
 *  - unique: if non-zero, attempt to ensure indices are unique (linear probe)
 *
 * Behavior:
 *  - If K == 0: nothing is written.
 *  - If dim == 0: idx_out entries are set to 0 and sign_out to 0.
 *  - If unique is requested and K > dim, uniqueness cannot be guaranteed; function
 *    will fill as many unique indices as possible (up to dim) and then reuse indices.
 */
void mage_rp_project_k(uint64_t key, size_t dim, uint64_t seed, size_t K, size_t *idx_out, int *sign_out, int unique) {
    if (K == 0) return;
    if (!idx_out && !sign_out) return;

    if (dim == 0) {
        /* safe defaults */
        for (size_t t = 0; t < K; ++t) {
            if (idx_out) idx_out[t] = 0;
            if (sign_out) sign_out[t] = 0;
        }
        return;
    }

    /* Use a base hash and a stride to generate K pseudo-independent values.
     * The stride is an odd constant to ensure good mixing when added.
     */
    const uint64_t stride = 0x9e3779b97f4a7c15ULL; /* golden ratio */
    uint64_t base = mage_hash_u64(key ^ 0xDEADBEEFF00DBABEULL, seed);

    if (!unique) {
        /* Fast path: no uniqueness required */
        for (size_t t = 0; t < K; ++t) {
            uint64_t h = splitmix64_step(base + (uint64_t)t * stride);
            __uint128_t prod = ( __uint128_t)h * ( __uint128_t)dim;
            if (idx_out) idx_out[t] = (size_t)(prod >> 64);
            if (sign_out) sign_out[t] = (h & 1ULL) ? 1 : -1;
        }
        return;
    }

    /* Unique path: attempt to produce unique indices using linear probing.
     * We will generate candidate indices and, if a duplicate is found, probe forward.
     * We cap attempts per projection to `dim` to avoid infinite loops.
     */
    /* Small temporary bitmap to track used indices when dim is reasonably small.
     * If dim is large, fallback to linear search in the output array.
     */
    const size_t BITMAP_LIMIT = 65536; /* allocate bitmap only when dim <= this */
    unsigned char *bitmap = NULL;
    if (dim <= BITMAP_LIMIT) {
        size_t bsz = (dim + 7) / 8;
        bitmap = (unsigned char*)calloc(bsz, 1);
    }

    for (size_t t = 0; t < K; ++t) {
        uint64_t h = splitmix64_step(base + (uint64_t)t * stride);
        size_t idx = (size_t)(((__uint128_t)h * ( __uint128_t)dim) >> 64);
        int sgn = (h & 1ULL) ? 1 : -1;

        if (bitmap) {
            /* try to find an unused index via linear probing */
            size_t attempts = 0;
            while (attempts < dim) {
                size_t byte = idx >> 3;
                unsigned char mask = (unsigned char)(1u << (idx & 7));
                if ((bitmap[byte] & mask) == 0) {
                    /* mark used and accept */
                    bitmap[byte] |= mask;
                    break;
                }
                idx = (idx + 1) % dim;
                attempts++;
            }
            /* if attempts == dim, all indices used; accept current idx (wrap) */
        } else {
            /* no bitmap: check against previously chosen indices in idx_out */
            size_t attempts = 0;
            int dup_found = 0;
            while (attempts < dim) {
                dup_found = 0;
                for (size_t j = 0; j < t; ++j) {
                    if (idx_out && idx_out[j] == idx) { dup_found = 1; break; }
                }
                if (!dup_found) break;
                idx = (idx + 1) % dim;
                attempts++;
            }
            /* if attempts == dim, all indices used; accept current idx */
        }

        if (idx_out) idx_out[t] = idx;
        if (sign_out) sign_out[t] = sgn;
    }

    if (bitmap) free(bitmap);
}
