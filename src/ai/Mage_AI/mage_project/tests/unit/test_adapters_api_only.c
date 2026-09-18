#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include "mage/api.h"

/* Build-time test: ensure presentation code compiles with only mage/api.h includes.
 * This test will compile and link as part of the unit tests; it performs a
 * runtime smoke exercise of API functions (using safe fallbacks where available).
 */

int main(void) {
    /* Test parse wrapper */
    char* p = mage_api_parse_prompt("hello test");
    if (p) { free(p); }

    /* Test context load (file likely missing) should return 0 or >0 without crash */
    int got = mage_api_context_load_from_jsonl("data/context_history.jsonl");
    (void)got;

    /* Probe mphf (may return 0) */
    int ok = mage_api_mphf_probe("data/mphf.bin");
    (void)ok;

    /* Train from file (likely missing) should return negative on error or >=0 */
    int t = mage_api_train_from_file("nonexistent-file.txt");
    (void)t;

    printf("adapter_api_only smoke OK\n");
    return 0;
}
