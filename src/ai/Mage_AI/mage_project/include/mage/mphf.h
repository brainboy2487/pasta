/*
 * mphf.h
 *
 * Public API for the Minimal Perfect Hash Function (MPHF) implementation
 * based on the CHD (Compress, Hash, Displace) algorithm.
 *
 * This provides a collision-free mapping for a fixed key set, guaranteeing
 * O(1) worst-case lookup time.
 *
 * Generated: 2026-01-23 15:32:35
 */
#ifndef MAGE_MPHF_H
#define MAGE_MPHF_H

#include <stdint.h>
#include <stddef.h>

/* ============================================================================
 * PUBLIC API STRUCTURES AND TYPES
 * ============================================================================ */

/**
 * Opaque structure for the Minimal Perfect Hash Function.
 * Holds all necessary state for lookups.
 */
typedef struct mphf_t mphf_t;

/* ============================================================================
 * PUBLIC API FUNCTIONS
 * ============================================================================ */

/**
 * Build a Minimal Perfect Hash Function from a given set of keys.
 *
 * @param keys An array of NUL-terminated strings.
 * @param num_keys The number of keys in the array.
 * @return A pointer to the generated MPHF structure, or NULL on failure.
 *         The caller is responsible for destroying the MPHF with mphf_destroy.
 */
mphf_t* mphf_build(const char* const* keys, size_t num_keys);

/**
 * Perform a lookup for a given key.
 *
 * For keys that were in the original key set, this function returns a unique
 * hash value in the range [0, num_keys-1].
 *
 * For keys not in the original set, the return value is not defined and may
 * collide with valid hashes.
 *
 * @param mphf A pointer to the MPHF structure.
 * @param key The NUL-terminated string key to look up.
 * @return The unique hash value for the key.
 */
uint32_t mphf_lookup(const mphf_t* mphf, const char* key);

/**
 * Destroy the MPHF and free all associated resources.
 *
 * @param mphf A pointer to the MPHF structure to be destroyed.
 */
void mphf_destroy(mphf_t* mphf);

/**
 * Serialize the MPHF to a binary file for fast loading later.
 * Format (native endian):
 *  - size_t num_keys
 *  - size_t num_buckets
 *  - uint64_t seed
 *  - int32_t displacements[num_buckets]
 *
 * @param mphf Pointer to the MPHF instance to serialize.
 * @param path Filesystem path to write to.
 * @return 0 on success, -1 on failure.
 */
int mphf_serialize(const mphf_t* mphf, const char* path);

/**
 * Load an MPHF from a previously serialized file.
 * @param path Filesystem path to read from.
 * @return Pointer to a newly allocated `mphf_t` or NULL on error.
 */
mphf_t* mphf_load(const char* path);

#endif /* MAGE_MPHF_H */

