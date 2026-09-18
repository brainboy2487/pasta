/* hash.h - Hash utility public API
 * Generated to provide prototypes expected by other modules.
 */
#ifndef MAGE_HASH_H
#define MAGE_HASH_H

#include <stdint.h>
#include <stddef.h>

/* 64-bit hash with seed parameter. Implementation is a small, portable
 * FNV-inspired mixer for deterministic behavior during testing.
 */
uint64_t hash_murmur64(const void* key, size_t len, uint64_t seed);

/* Deterministic sign function used by projection code. Returns +1 or -1. */
int hash_sign64(uint64_t x);

#endif /* MAGE_HASH_H */
