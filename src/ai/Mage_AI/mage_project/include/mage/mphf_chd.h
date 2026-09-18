/* mphf_chd.h
 * CHD-style MPHF public header
 */
#ifndef MAGE_MPHF_CHD_H
#define MAGE_MPHF_CHD_H

#include <stdint.h>
#include <stddef.h>

typedef struct mphf_chd_t mphf_chd_t;

mphf_chd_t* mphf_chd_build(const char* const* keys, size_t num_keys);
uint32_t mphf_chd_lookup(const mphf_chd_t* mphf, const char* key);
void mphf_chd_destroy(mphf_chd_t* mphf);
int mphf_chd_serialize(const mphf_chd_t* mphf, const char* path);
mphf_chd_t* mphf_chd_load(const char* path);

#endif /* MAGE_MPHF_CHD_H */
