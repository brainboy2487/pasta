/* mphf_stub.c - deterministic fallback hash-based mapping
 * Replace with a generated MPHF for production; this stub provides a stable
 * mapping into [0, space) using a deterministic 64-bit mix.
 */

#include <stdint.h>


static uint64_t rotl64(uint64_t x, int r) __attribute__((unused));
static uint64_t rotl64(uint64_t x, int r) { return (x << r) | (x >> (64 - r)); }

/* FNV-1a 64-bit mix as a deterministic stand-in */
static uint64_t fnv1a64(const char* s) {
    uint64_t h = 14695981039346656037ULL;
    while (*s) {
        h ^= (unsigned char)*s++;
        h *= 1099511628211ULL;
    }
    /* avalanche */
    h ^= h >> 33; h *= 0xff51afd7ed558ccdULL; h ^= h >> 33; h *= 0xc4ceb9fe1a85ec53ULL; h ^= h >> 33;
    return h;
}

/* Internal stub mapping - does not export `mphf_lookup` to avoid colliding
 * with a real `mphf.c` implementation. Use this helper for local testing.
 */
uint32_t mphf_stub_lookup(const char* key, uint32_t space) {
    if (!key || space == 0) return 0;
    uint64_t h = fnv1a64(key);
    return (uint32_t)(h % (uint64_t)space);
}
