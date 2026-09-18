#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "mage/mphf.h"

int main(int argc, char** argv) {
    if (argc < 3) {
        fprintf(stderr, "Usage: %s <mphf_file> <key>\n", argv[0]);
        return 2;
    }
    const char* path = argv[1];
    const char* key = argv[2];
    mphf_t* m = mphf_load(path);
    if (!m) { fprintf(stderr, "Failed to load MPHF from %s\n", path); return 1; }
    uint32_t idx = mphf_lookup(m, key);
    printf("Lookup '%s' -> %u\n", key, idx);
    mphf_destroy(m);
    return 0;
}
