/* hash.c - Hash Utilities
 * Generated: 2026-01-23 15:32:35
 */

#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include "mage/hash.h"

/* Simple deterministic 64-bit mixer used as a stand-in for MurmurHash64.
 * This implementation is portable and fast for tests; replace with a
 * production-grade MurmurHash or xxhash as needed.
 */
uint64_t hash_murmur64(const void* key, size_t len, uint64_t seed) {
    const unsigned char* data = (const unsigned char*)key;
    uint64_t h = 14695981039346656037ULL ^ seed;
    for (size_t i = 0; i < len; ++i) {
        h ^= (uint64_t)data[i];
        h *= 1099511628211ULL;
        /* avalanche */
        h ^= (h >> 23) ^ (h << 7);
    }
    /* final mix */
    h ^= h >> 33;
    h *= 0xff51afd7ed558ccdULL;
    h ^= h >> 33;
    return h;
}

int hash_sign64(uint64_t x) {
    /* Return +1 or -1 deterministically based on low bit parity. */
    return (x & 1ULL) ? 1 : -1;
}
