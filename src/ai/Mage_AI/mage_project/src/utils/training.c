/* training.c - Minimal training pipeline implementation
 * Aligns with the vocabulary protocol: builds vocab JSONL with required fields,
 * generates an MPHF artifact, and invokes the training ingestion.
 */

#define _POSIX_C_SOURCE 200809L

#include "mage/training.h"
#include "mage/api.h"
#include "mage/log.h"
#include "mage/subproc.h"
#include "mage/tool.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int mage_training_run_pipeline(const char* input_path) {
    if (!input_path) return -1;
    const char* vocab_out = "data/vocab/generated_vocab.jsonl";
    mage_log(MAGE_LOG_INFO, "training: building vocab from %s -> %s", input_path, vocab_out);
    int wrote = mage_api_build_vocab_from_file(input_path, vocab_out, 1);
    if (wrote < 0) {
        mage_log(MAGE_LOG_ERROR, "training: build_vocab failed for %s", input_path);
        return -2;
    }
    mage_log(MAGE_LOG_INFO, "training: built vocab (%d tokens)", wrote);

    /* Generate MPHF for the produced vocab to speed up TAP lookups */
    char cmd[1024];
    char* tool = NULL;
    tool = mage_find_tool("tools/gen_mphf");
    if (tool) {
        snprintf(cmd, sizeof(cmd), "%s %s data/mphf.bin", tool, vocab_out);
        free(tool);
    } else {
        snprintf(cmd, sizeof(cmd), "./tools/gen_mphf %s data/mphf.bin", vocab_out);
    }
    mage_log(MAGE_LOG_INFO, "training: generating MPHF via: %s", cmd);
    int rc = subproc_run_shell(cmd, 120000);
    if (rc != 0) {
        mage_log(MAGE_LOG_WARN, "training: MPHF generation returned rc=%d", rc);
    } else {
        mage_log(MAGE_LOG_INFO, "training: MPHF generated -> data/mphf.bin");
    }

    /* Finally, run the training ingestion (existing API) */
    mage_log(MAGE_LOG_INFO, "training: invoking mage_api_train_from_file(%s)", input_path);
    int added = mage_api_train_from_file(input_path);
    if (added < 0) {
        mage_log(MAGE_LOG_ERROR, "training: mage_api_train_from_file failed for %s", input_path);
        return -3;
    }
    mage_log(MAGE_LOG_INFO, "training: training ingestion added %d entries", added);
    return added;
}
