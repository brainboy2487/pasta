#include <stdlib.h>
#include <string.h>
#include "mage/mphf_loader.h"
#include "mage/mphf_chd.h"
#include "mage/mphf.h"

struct mphf_loader_t {
    int kind; /* 0 = none, 1 = chd, 2 = legacy */
    void* mphf; /* either mphf_chd_t* or mphf_t* */
    char* path; /* path used to load (for diagnostics) */
};

static const char* env_override(void) {
    const char* e = getenv("MAGE_MPHF_PATH");
    return (e && e[0]) ? e : NULL;
}

mphf_loader_t* mphf_loader_load(const char* path) {
    const char* try = env_override();
    if (!try) try = path;
    if (!try) return NULL;

    mphf_loader_t* l = calloc(1, sizeof(*l));
    if (!l) return NULL;

    l->path = malloc(strlen(try) + 1);
    if (!l->path) { free(l); return NULL; }
    memcpy(l->path, try, strlen(try) + 1);

    /* Try CHD loader first */
    mphf_chd_t* c = mphf_chd_load(try);
    if (c) {
        l->kind = 1;
        l->mphf = (void*)c;
        return l;
    }

    /* Try legacy loader */
    mphf_t* m = mphf_load(try);
    if (m) {
        l->kind = 2;
        l->mphf = (void*)m;
        return l;
    }

    /* No loader succeeded; free and return NULL */
    free(l->path);
    free(l);
    return NULL;
}

uint32_t mphf_loader_lookup(const mphf_loader_t* loader, const char* key) {
    if (!loader || !key) return UINT32_MAX;
    if (loader->kind == 1) {
        mphf_chd_t* c = (mphf_chd_t*)loader->mphf;
        if (!c) return UINT32_MAX;
        return mphf_chd_lookup(c, key);
    }
    if (loader->kind == 2) {
        mphf_t* m = (mphf_t*)loader->mphf;
        if (!m) return UINT32_MAX;
        return mphf_lookup(m, key);
    }
    return UINT32_MAX;
}

void mphf_loader_destroy(mphf_loader_t* loader) {
    if (!loader) return;
    if (loader->kind == 1) {
        mphf_chd_destroy((mphf_chd_t*)loader->mphf);
    } else if (loader->kind == 2) {
        mphf_destroy((mphf_t*)loader->mphf);
    }
    free(loader->path);
    free(loader);
}

int mphf_loader_probe(const char* path) {
    const char* try = env_override();
    if (!try) try = path;
    if (!try) return 0;
    /* Try CHD first */
    mphf_chd_t* c = mphf_chd_load(try);
    if (c) { mphf_chd_destroy(c); return 1; }
    mphf_t* m = mphf_load(try);
    if (m) { mphf_destroy(m); return 1; }
    return 0;
}
