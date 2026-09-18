#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>

/* Simple vectors.bin writer for testing mmapped index.
 * Usage: write_vectors <path> <n> <d> <mode>
 * modes:
 *  normal     - write full header + n*d float32 sequential values
 *  truncate   - write header then only part of data (half)
 *  badheader  - write header with d=0
 *  misalign   - write header then an extra 3 bytes before data
 */

static void usage(const char *prog) {
    fprintf(stderr, "Usage: %s <path> <n> <d> <mode>\n", prog);
    exit(2);
}

int main(int argc, char **argv) {
    if (argc < 5) usage(argv[0]);
    const char *path = argv[1];
    uint32_t n = (uint32_t)atoi(argv[2]);
    uint32_t d = (uint32_t)atoi(argv[3]);
    const char *mode = argv[4];

    FILE *f = fopen(path, "wb");
    if (!f) { perror("fopen"); return 1; }

    if (strcmp(mode, "badheader") == 0) {
        uint32_t z = n;
        uint32_t zz = 0;
        fwrite(&z, sizeof(z), 1, f);
        fwrite(&zz, sizeof(zz), 1, f);
        fclose(f);
        return 0;
    }

    /* normal or other modes: write header */
    fwrite(&n, sizeof(n), 1, f);
    fwrite(&d, sizeof(d), 1, f);

    /* misalign: write 3 bytes of padding to disrupt expected alignment */
    if (strcmp(mode, "misalign") == 0) {
        unsigned char pad[3] = {0xAA, 0xBB, 0xCC};
        fwrite(pad, 1, sizeof(pad), f);
    }

    size_t total = (size_t)n * (size_t)d;
    for (size_t i = 0; i < total; ++i) {
        float v = (float)(i % 257) * 0.001f; /* reproducible pattern */
        fwrite(&v, sizeof(v), 1, f);
        if (strcmp(mode, "truncate") == 0 && i >= total/2) break;
    }

    fclose(f);
    return 0;
}
