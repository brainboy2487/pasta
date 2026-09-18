/* rp.h - deterministic random projection helpers for Mage
 * Generated: 2026-01-25
 */
#ifndef MAGE_RP_H
#define MAGE_RP_H

#include <stddef.h>
#include <stdint.h>

/* Hash a 64-bit value with a seed producing a 64-bit pseudorandom output. */
uint64_t mage_hash_u64(uint64_t x, uint64_t seed);

/* Return deterministic sign (+1 or -1) for a key and seed. */
int mage_hash_sign64(uint64_t key, uint64_t seed);

/* Map a 64-bit key into a projection index in range [0, dim) deterministically. */
size_t mage_rp_index(uint64_t key, size_t dim, uint64_t seed);

/* Project a key into (index, sign) pair for sparse random projection.
 * `dim` must be > 0. Results are deterministic for given key/seed.
 */
void mage_rp_project(uint64_t key, size_t dim, uint64_t seed, size_t *idx_out, int *sign_out);

/* Project a key into K (index, sign) pairs. Writes up to K entries into
 * `idx_out` and `sign_out`. If `unique` is non-zero, attempts to produce
 * unique indices by linear probing; duplicates may occur if `dim` < K.
 */
void mage_rp_project_k(uint64_t key, size_t dim, uint64_t seed, size_t K, size_t *idx_out, int *sign_out, int unique);

#endif /* MAGE_RP_H */
#ifndef MAGE_RP_H
#define MAGE_RP_H

#include "types.h"

int rp_project(const double* matrix, size_t n, const rp_config_t* config);

#endif
