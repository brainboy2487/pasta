/*
 * test_mphf.c
 *
 * Unit tests for the Minimal Perfect Hash Function (MPHF) implementation.
 * This ensures that the generated hash function is indeed perfect (no collisions)
 * and that lookups are correct.
 *
 * Generated: 2026-01-23 15:32:35
 */

#include "mage/mphf.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

/* ============================================================================
 * TEST FIXTURES AND SETUP
 * ============================================================================ */

/* A sample key set for testing */
const char* test_keys[] = {
    "apple", "banana", "cherry", "date", "elderberry",
    "fig", "grape", "honeydew", "kiwi", "lemon"
};
const size_t num_test_keys = sizeof(test_keys) / sizeof(test_keys[0]);

/* ============================================================================ 
 * TEST CASES
 * ============================================================================ */

/**
 * Test case 1: Build MPHF and verify no collisions
 */
static void test_build_and_lookup() {
    printf("test_build_and_lookup...\n");

    /* 1. Build the MPHF from the key set */
    mphf_t* mphf = mphf_build(test_keys, num_test_keys);
    assert(mphf != NULL && "MPHF build failed");

    /* 2. Verify that each key maps to a unique value in [0, num_keys-1] */
    uint32_t* seen_values = calloc(num_test_keys, sizeof(uint32_t));
    assert(seen_values != NULL && "Memory allocation failed");

    for (size_t i = 0; i < num_test_keys; ++i) {
        uint32_t value = mphf_lookup(mphf, test_keys[i]);
        
        /* Check if value is in the correct range */
        assert(value < num_test_keys && "Hash value out of range");

        /* Check for collisions */
        assert(seen_values[value] == 0 && "Collision detected!");
        
        seen_values[value] = 1;
    }

    /* 3. Clean up */
    free(seen_values);
    mphf_destroy(mphf);

    printf("...PASSED\n");
}

/**
 * Test case 2: Test lookup of non-existent keys
 */
static void test_non_existent_keys() {
    printf("test_non_existent_keys...\n");
    
    /* Build MPHF */
    mphf_t* mphf = mphf_build(test_keys, num_test_keys);
    assert(mphf != NULL);

    /*
     * Note: The behavior for keys not in the original set is undefined for a
     * standard MPHF. A robust implementation might return a special value or
     * have a higher probability of collision for non-set keys.
     *
     * For a CHD implementation, a non-member key will likely produce a hash
     * value, which may or may not collide with a valid hash. The critical
     * guarantee is only for the key set it was built with.
     *
     * We can verify this by checking if a few "unknown" keys produce values
     * that are either out of range or flagged as invalid if the MPHF supports it.
     */

    const char* non_keys[] = {"strawberry", "pineapple", "mango"};
    size_t num_non_keys = sizeof(non_keys) / sizeof(non_keys[0]);

    for (size_t i = 0; i < num_non_keys; ++i) {
        /* The result of looking up a non-key is not strictly defined,
         * but we can ensure it doesn't crash.
         */
        mphf_lookup(mphf, non_keys[i]);
    }
    
    mphf_destroy(mphf);
    
    printf("...PASSED (no crash)\n");
}


/* ============================================================================
 * TEST RUNNER
 * ============================================================================ */

int main() {
    printf("--- Running MPHF Unit Tests ---\n");
    
    test_build_and_lookup();
    test_non_existent_keys();
    
    printf("-----------------------------\n");
    printf("All MPHF tests passed!\n");
    
    return 0;
}
