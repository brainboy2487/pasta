/* Unit test for CHD-style MPHF */
#include "mage/mphf_chd.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

const char* test_keys[] = {"apple","banana","cherry","date","elderberry","fig","grape","honeydew","kiwi","lemon"};
const size_t num_test_keys = sizeof(test_keys)/sizeof(test_keys[0]);

static void test_build_and_lookup() {
    printf("test_mphf_chd build_and_lookup...\n");
    mphf_chd_t* m = mphf_chd_build(test_keys, num_test_keys);
    assert(m != NULL);
    uint32_t *seen = calloc(num_test_keys, sizeof(uint32_t));
    assert(seen);
    for (size_t i=0;i<num_test_keys;++i) {
        uint32_t v = mphf_chd_lookup(m, test_keys[i]);
        assert(v < num_test_keys);
        assert(seen[v] == 0);
        seen[v] = 1;
    }
    free(seen);
    mphf_chd_destroy(m);
    printf("...PASSED\n");
}

int main() {
    printf("--- Running CHD MPHF Unit Test ---\n");
    test_build_and_lookup();
    printf("All CHD MPHF tests passed!\n");
    return 0;
}
