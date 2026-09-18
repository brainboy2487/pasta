#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "mage/manifest.h"

int main(void) {
    const char* tmp = "build/tests/manifest_test.jsonl";
    FILE* f = fopen(tmp, "w");
    if (!f) return 1;
    fprintf(f, "{\"index\": 0, \"id\": \"doc0\", \"title\": \"First\"}\n");
    fprintf(f, "{\"index\": 5, \"id\": \"doc5\", \"path\": \"/tmp/doc5.txt\"}\n");
    fclose(f);

    manifest_t* m = manifest_load(tmp);
    if (!m) { printf("failed to load manifest\n"); return 2; }
    if (m->n != 2) { printf("expected 2 entries got %u\n", m->n); manifest_free(m); return 3; }
    const manifest_entry_t* e0 = manifest_get(m, 0);
    if (!e0 || strcmp(e0->id, "doc0") != 0) { printf("entry0 mismatch\n"); manifest_free(m); return 4; }
    const manifest_entry_t* e5 = manifest_get(m, 5);
    if (!e5 || strcmp(e5->id, "doc5") != 0) { printf("entry5 mismatch\n"); manifest_free(m); return 5; }
    manifest_free(m);
    printf("manifest tests OK\n");
    return 0;
}
