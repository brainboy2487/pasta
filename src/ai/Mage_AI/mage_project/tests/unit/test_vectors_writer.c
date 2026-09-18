#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include "mage/mmappedindex.h"
#include "mage/subproc.h"

static int run_writer_and_test(const char* mode) {
    const char* path = "data/test_vectors.bin";
    char cmd[1024];
    snprintf(cmd, sizeof(cmd), "gcc -O2 -Iinclude tools/write_vectors.c -o tools/write_vectors && tools/write_vectors %s 10 8 %s", path, mode);
    int rc = subproc_run_shell(cmd, 120000);
    if (rc != 0) { fprintf(stderr, "writer failed (rc=%d)\n", rc); return 2; }

    mmapped_index_t *idx = mmapped_index_open(path, 8, sizeof(float)*8);
    if (!idx) { fprintf(stderr, "mmapped_index_open returned NULL for mode=%s\n", mode); return 3; }

    float vec[8];
    int r = mmapped_index_get(idx, 0, vec);
    if (strcmp(mode, "normal") == 0) {
        if (r != 0) { fprintf(stderr, "expected success for normal mode\n"); mmapped_index_close(idx); return 4; }
        /* verify pattern written by writer: first float = 0.0 */
        if (vec[0] != 0.0f) { fprintf(stderr, "unexpected value vec[0]=%f\n", vec[0]); mmapped_index_close(idx); return 5; }
    } else {
        /* fallback cases: we expect either success with zero vector, or failure (-1) */
        if (r != 0 && r != -1) { fprintf(stderr, "unexpected rc %d for mode=%s\n", r, mode); mmapped_index_close(idx); return 6; }
    }

    mmapped_index_close(idx);
    return 0;
}

int main(void) {
    int rc = 0;
    rc += run_writer_and_test("normal");
    rc += run_writer_and_test("truncate");
    rc += run_writer_and_test("badheader");
    rc += run_writer_and_test("misalign");
    if (rc == 0) printf("vectors writer tests: PASSED\n");
    return rc;
}
