/* mphf_loader.h
 * Canonical MPHF loader wrapper that prefers CHD-style serialized MPHFs
 * but falls back to legacy loaders. It also supports an environment
 * variable override `MAGE_MPHF_PATH` to specify a path at runtime.
 */
#ifndef MAGE_MPHF_LOADER_H
#define MAGE_MPHF_LOADER_H

#include <stdint.h>
#include <stddef.h>

typedef struct mphf_loader_t mphf_loader_t;

/* Load an MPHF from `path`. If the environment variable `MAGE_MPHF_PATH`
 * is set, its value will be tried first. The loader will attempt CHD
 * format first (if available) and then the legacy loader. Returns NULL
 * on failure.
 */
mphf_loader_t* mphf_loader_load(const char* path);

/* Lookup a key using the loaded MPHF. Returns UINT32_MAX on failure. */
uint32_t mphf_loader_lookup(const mphf_loader_t* loader, const char* key);

/* Destroy and free resources. Safe to call with NULL. */
void mphf_loader_destroy(mphf_loader_t* loader);

/* Probe whether a path is loadable by any supported loader. Returns 1 if
 * loadable, 0 otherwise.
 */
int mphf_loader_probe(const char* path);

#endif /* MAGE_MPHF_LOADER_H */
