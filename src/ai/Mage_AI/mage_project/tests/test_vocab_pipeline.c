/* test_vocab_pipeline.c - integration test for vocab -> MPHF -> loader */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "mage/api.h"
#include "mage/mphf.h"
#include "mage/tool.h"
#include <unistd.h>

int main(void) {
    const char* input = "data/train_sample.txt";
    const char* out_vocab = "data/vocab/generated_vocab.jsonl";
    const char* mphf_out = "data/mphf.test.bin";

    /* build vocab */
    int wrote = mage_api_build_vocab_from_file(input, out_vocab, 1);
    if (wrote <= 0) { fprintf(stderr, "vocab build failed or produced no tokens\n"); return 2; }

    /* run gen_mphf tool via resolved path if present */
    char cmd[512];
    char* tool = mage_find_tool("tools/gen_mphf");
    if (tool) {
        snprintf(cmd, sizeof(cmd), "%s data/vocab/generated_vocab.jsonl data/mphf.test.bin", tool);
        free(tool);
    } else {
        snprintf(cmd, sizeof(cmd), "./tools/gen_mphf data/vocab/generated_vocab.jsonl data/mphf.test.bin");
    }
    if (system(cmd) != 0) { fprintf(stderr, "gen_mphf failed\n"); return 3; }

    /* probe load via API */
    int ok = mage_api_mphf_probe(mphf_out);
    if (!ok) {
        fprintf(stderr, "mphf probe failed\n"); return 4;
    }

    /* cleanup */
    unlink(mphf_out);
    printf("OK\n");
    return 0;
}
