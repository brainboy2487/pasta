/*
 * cooc.c - Sparse Cooccurrence Structure
 * ========================================
 * Implements efficient tracking of token cooccurrence events for building
 * the local stochastic tensor during document processing.
 *
 * Uses Robin Hood hashing for fast insertion and lookup with 64-bit
 * composite keys: (i << 32) | (j << 2) | context_bits
 *
 * Generated: {timestamp}
 */

#include "mage/cooc.h"
#include "mage/types.h"
#include <stdlib.h>
#include <string.h>

/* ============================================================================
 * ROBIN HOOD HASH TABLE CONFIGURATION
 * ============================================================================ */

#define COOC_INITIAL_CAPACITY 4096
#define COOC_LOAD_FACTOR 0.75
#define COOC_PROBE_LIMIT 256


/* ============================================================================
 * HASH FUNCTIONS
 * ============================================================================ */

/**
 * Hash function for 64-bit composite key
 * Uses MurmurHash3-style finalizer for good distribution
 */
static inline uint64_t cooc_hash64(uint64_t key) {
    /* TODO: Implement high-quality 64-bit hash
     * 
     * Requirements:
     * - Deterministic (no randomness)
     * - Good avalanche properties
     * - Fast (< 10 CPU cycles)
     * 
     * Suggestions:
     * - MurmurHash3 finalizer
     * - xxHash64
     * - FNV-1a
     */
    
    key ^= key >> 33;
    key *= 0xff51afd7ed558ccdULL;
    key ^= key >> 33;
    key *= 0xc4ceb9fe1a85ec53ULL;
    key ^= key >> 33;
    
    return key;
}

/**
 * Create composite key from cooccurrence indices
 * 
 * Key layout (64 bits):
 * - Bits 63-32: from_state (i)
 * - Bits 31-2: to_state (j)
 * - Bits 1-0: context bits (e.g., sentence boundary, paragraph marker)
 */
static inline uint64_t cooc_make_key(uint32_t from_state, 
                      uint32_t to_state,
                      uint8_t context_bits) {
    return ((uint64_t)from_state << 32) | 
        ((uint64_t)to_state << 2) | 
        (context_bits & 0x3);
}


/* ============================================================================
 * COOCCURRENCE TABLE OPERATIONS
 * ============================================================================ */

/**
 * Create new cooccurrence table
 */
static size_t cooc_next_pow2(size_t v) {
    size_t r = 1;
    while (r < v) r <<= 1;
    return r;
}

cooc_table_t* cooc_table_create(size_t initial_capacity) {
    if (initial_capacity == 0) initial_capacity = COOC_INITIAL_CAPACITY;
    size_t cap = cooc_next_pow2(initial_capacity);
    if (cap < 8) cap = 8;

    cooc_table_t* table = calloc(1, sizeof(cooc_table_t));
    if (!table) return NULL;

    table->entries = calloc(cap, sizeof(cooc_entry_t));
    if (!table->entries) { free(table); return NULL; }

    table->capacity = cap;
    table->size = 0;
    table->load_factor = COOC_LOAD_FACTOR;
    return table;
}

/**
 * Destroy cooccurrence table
 */
void cooc_table_destroy(cooc_table_t* table) {
    if (!table) return;
    free(table->entries);
    free(table);
}

/**
 * Insert or increment cooccurrence count
 * 
 * Uses Robin Hood hashing for fast insertion with good cache locality.
 * 
 * @param table Cooccurrence table
 * @param from_state Source state index
 * @param to_state Target state index
 * @param context_bits Context flags (2 bits)
 * @return 0 on success, error code otherwise
 */
static int cooc_table_resize(cooc_table_t* table, size_t new_cap) {
    if (!table) return -1;
    new_cap = cooc_next_pow2(new_cap);
    if (new_cap < 8) new_cap = 8;

    cooc_entry_t* old_entries = table->entries;
    size_t old_cap = table->capacity;

    cooc_entry_t* new_entries = calloc(new_cap, sizeof(cooc_entry_t));
    if (!new_entries) return -1;

    /* Re-insert old entries into new table using robust Robin Hood insertion */
    size_t new_size = 0;
    size_t mask = new_cap - 1;

    for (size_t i = 0; i < old_cap; ++i) {
        if (old_entries[i].count == 0) continue;
        uint64_t key = old_entries[i].key;
        uint32_t count = old_entries[i].count;

        uint64_t h = cooc_hash64(key);
        size_t idx = (size_t)(h & mask);
        size_t dist = 0;

        for (size_t probe = 0; probe < COOC_PROBE_LIMIT; ++probe) {
            if (new_entries[idx].count == 0) {
                new_entries[idx].key = key;
                new_entries[idx].count = count;
                new_entries[idx].distance_from_ideal = (uint16_t)dist;
                new_size++;
                break;
            }

            size_t ideal = (size_t)(cooc_hash64(new_entries[idx].key) & mask);
            size_t curdist = (idx + new_cap - ideal) & mask;
            if (curdist < dist) {
                /* swap - place incoming here and continue with displaced */
                uint64_t k2 = new_entries[idx].key;
                uint32_t c2 = new_entries[idx].count;
                uint16_t d2 = new_entries[idx].distance_from_ideal;

                new_entries[idx].key = key;
                new_entries[idx].count = count;
                new_entries[idx].distance_from_ideal = (uint16_t)dist;

                /* Continue inserting displaced entry */
                key = k2; count = c2; dist = d2;
            }

            idx = (idx + 1) & mask;
            dist++;
        }
    }

    /* Swap in new table */
    table->entries = new_entries;
    table->capacity = new_cap;
    table->size = new_size;

    free(old_entries);
    return 0;
}

int cooc_table_insert(cooc_table_t* table,
                     uint32_t from_state,
                     uint32_t to_state,
                     uint8_t context_bits) {
    if (!table) return -1;

    uint64_t key = cooc_make_key(from_state, to_state, context_bits);

    /* Resize if necessary */
    if ((double)(table->size + 1) > (double)table->capacity * table->load_factor) {
        if (cooc_table_resize(table, table->capacity * 2) != 0) return -1;
    }

    uint64_t h = cooc_hash64(key);
    size_t mask = table->capacity - 1;
    size_t idx = (size_t)(h & mask);
    size_t dist = 0;

    for (size_t probe = 0; probe < COOC_PROBE_LIMIT; ++probe) {
        if (table->entries[idx].count == 0) {
            /* empty slot */
            table->entries[idx].key = key;
            table->entries[idx].count = 1;
            table->entries[idx].distance_from_ideal = (uint16_t)dist;
            table->size++;
            return 0;
        }

        if (table->entries[idx].key == key) {
            table->entries[idx].count++;
            return 0;
        }

        size_t ideal = (size_t)(cooc_hash64(table->entries[idx].key) & mask);
        size_t curdist = (idx + table->capacity - ideal) & mask;
        if (curdist < dist) {
            /* Robin Hood: swap the current entry with the incoming one */
            uint64_t k2 = table->entries[idx].key;
            uint16_t d2 = table->entries[idx].distance_from_ideal;

            table->entries[idx].key = key;
            table->entries[idx].count = 1;
            table->entries[idx].distance_from_ideal = (uint16_t)dist;

            /* continue inserting displaced entry */
            key = k2; /* displaced key */
            dist = d2; /* start with displaced's distance */
        }

        idx = (idx + 1) & mask;
        dist++;
    }

    /* probe limit reached; treat as failure */
    return -1;
}

/**
 * Query cooccurrence count
 */
uint32_t cooc_table_get(const cooc_table_t* table,
                       uint32_t from_state,
                       uint32_t to_state,
                       uint8_t context_bits) {
    if (!table) return 0;
    uint64_t key = cooc_make_key(from_state, to_state, context_bits);
    uint64_t h = cooc_hash64(key);
    size_t mask = table->capacity - 1;
    size_t idx = (size_t)(h & mask);
    size_t dist = 0;

    for (size_t probe = 0; probe < COOC_PROBE_LIMIT; ++probe) {
        if (table->entries[idx].count == 0) return 0;
        if (table->entries[idx].key == key) return table->entries[idx].count;

        size_t ideal = (size_t)(cooc_hash64(table->entries[idx].key) & mask);
        size_t curdist = (idx + table->capacity - ideal) & mask;
        if (curdist < dist) return 0; /* early termination */

        idx = (idx + 1) & mask;
        dist++;
    }

    return 0;
}

/* EOF */
