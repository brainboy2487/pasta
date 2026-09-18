#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include "mage/api.h"

int main(void) {
    if (mage_api_init() != 0) {
        fprintf(stderr, "mage init failed\n");
        return 1;
    }

    /* Train using Python pipeline when available to avoid C trainer instability */
    if (access("python/tools/train_pipeline.py", F_OK) == 0) {
        char cmd[1024];
        snprintf(cmd, sizeof(cmd), "python3 python/tools/train_pipeline.py --input %s --vocab-out data/vocab/generated_vocab.jsonl", "data/corpus/scraped.jsonl");
        int rc = system(cmd);
        printf("python train pipeline returned %d\n", rc);
        if (rc != 0) {
            fprintf(stderr, "training pipeline failed (rc=%d)\n", rc);
        }
    } else {
        /* Fallback: call C API trainer */
        int t = mage_api_train_from_file("data/corpus/scraped.jsonl");
        printf("train returned %d\n", t);
    }

    /* Generate a response */
    char* out = mage_api_generate("hello from example");
    if (out) {
        printf("generated: %s\n", out);
        free(out);
    }

    mage_api_shutdown();
    return 0;
}
