/*
 * contract.h - Lazy Block Contraction Interface
 * ===============================================
 * Public API for tiered tensor contraction.
 *
 * Generated: 2026-01-23 15:32:35
 */

#ifndef MAGE_CONTRACT_H
#define MAGE_CONTRACT_H

#include "types.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Main contraction function */
int contract_lazy_block(const mage_slice_t* slices,
                        const uint32_t* shortlist,
                        size_t shortlist_count,
                        size_t row_start,
                        size_t col_start,
                        size_t block_size,
                        double* output);

/* Cache management for block contraction */
int contract_set_cache_capacity(size_t entries);
void contract_flush_cache(void);

/* Precompute and cache a single block for a given slice */
int contract_precompute_block(const mage_slice_t* slices,
                             uint32_t slice_idx,
                             size_t row_start,
                             size_t col_start,
                             size_t block_size);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_CONTRACT_H */
