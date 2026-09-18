/*
 * types.h - MAGE Core Type Definitions
 * =====================================
 * Central header defining all core data structures, enums, and type aliases
 * used throughout the MAGE system.
 *
 * Generated: 2026-01-23 15:32:35
 */

#ifndef MAGE_TYPES_H
#define MAGE_TYPES_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

/* ============================================================================
 * ERROR CODES
 * ============================================================================ */

#define MAGE_SUCCESS 0
#define MAGE_ERROR_INVALID_ARGS -1
#define MAGE_ERROR_ALLOCATION_FAILED -2
#define MAGE_ERROR_IO_ERROR -3
#define MAGE_ERROR_BUDGET_EXCEEDED -4


/* ============================================================================
 * TIER ENUMERATION
 * ============================================================================ */

typedef enum {
    TIER_HOT = 0,    /* Dense storage, ~0.66 ns/elem */
    TIER_WARM = 1,   /* Low-rank factorization, ~7.1 ns/elem */
    TIER_COLD = 2    /* Analytic/separable, ~38.7 ns/elem */
} mage_tier_t;


/* ============================================================================
 * COLD TIER FUNCTION TYPES
 * ============================================================================ */

typedef enum {
    COLD_FUNC_POLYNOMIAL,
    COLD_FUNC_GAUSSIAN_MIXTURE,
    COLD_FUNC_CHEBYSHEV,
    COLD_FUNC_FIXED_POINT_TABLE
} cold_func_type_t;

typedef struct {
    cold_func_type_t type;
    double* coefficients;
    size_t num_coeffs;
    /* Additional parameters based on type */
} cold_func_params_t;


/* ============================================================================
 * SLICE DESCRIPTOR
 * ============================================================================ */

typedef struct {
    mage_tier_t tier;
    uint32_t slice_idx;
    double weight;           /* W_alpha: weight mass */
    size_t n;               /* State space dimension */
    
    union {
        struct {
            double* dense_matrix;  /* n×n row-major */
        } hot;
        
        struct {
            double* U;             /* n×r row-major */
            double* V_transposed;  /* r×n (V^T stored) */
            size_t rank;
        } warm;
        
        struct {
            cold_func_params_t f_params;  /* Row function f(i) */
            cold_func_params_t g_params;  /* Column function g(j) */
            cold_func_type_t func_type;
        } cold;
    } data;
} mage_slice_t;


/* ============================================================================
 * TENSOR METADATA
 * ============================================================================ */

typedef struct {
    size_t num_slices;      /* Total K slices */
    size_t state_dim;       /* n: state space size */
    double* weights;        /* Array of W_alpha values */
    uint32_t* recency;      /* Recency scores per slice */
    double* tier_costs;     /* Cost per tier (3 elements) */
} mage_tensor_metadata_t;


/* ============================================================================
 * TAP STRUCTURES
 * ============================================================================ */

typedef struct {
    uint32_t current_state;
    mage_tensor_metadata_t* tensor_meta;
    double confidence_threshold;
    uint32_t rng_seed;
    /* Additional context data */
    /* RP parameters used by TAP shortlist: number of projections and seed */
    size_t rp_K;
    uint64_t rp_seed;
} tap_context_t;

typedef struct {
    uint32_t ops_used;
    size_t shortlist_size;
    bool early_commit;
    double confidence_delta;
} tap_metrics_t;

typedef struct {
    size_t shortlist_size;
    double confidence_threshold;
    bool enable_early_commit;
    /* RP tuning parameters */
    size_t rp_K;        /* number of RP projections to use during shortlist */
    uint64_t rp_seed;   /* seed for RP hashing */
} tap_config_t;


/* ============================================================================
 * TAP ERROR CODES
 * ============================================================================ */

#define TAP_SUCCESS 0
#define TAP_ERROR_INVALID_ARGS -1
#define TAP_ERROR_UNKNOWN_FEATURE -2
#define TAP_ERROR_BUDGET_EXCEEDED -3


/* ============================================================================
 * CONTRACTION ERROR CODES
 * ============================================================================ */

#define CONTRACT_SUCCESS 0
#define CONTRACT_ERROR_INVALID_ARGS -1
#define CONTRACT_ERROR_ALLOCATION_FAILED -2
#define CONTRACT_ERROR_INVALID_TIER -3


/* ============================================================================
 * COOCCURRENCE TABLE
 * ============================================================================ */

typedef struct {
    uint64_t key;
    uint32_t count;
    uint16_t distance_from_ideal;  /* Robin Hood hashing */
} cooc_entry_t;

typedef struct {
    cooc_entry_t* entries;
    size_t capacity;
    size_t size;
    double load_factor;
} cooc_table_t;


/* ============================================================================
 * RANDOM PROJECTION PARAMETERS
 * ============================================================================ */

typedef struct {
    size_t rp_dim;          /* Target dimension (e.g., 128) */
    uint32_t hash_seed;     /* Seed for deterministic sign hashing */
    double* projection;     /* Output RP vector */
} rp_config_t;


/* ============================================================================
 * VECTOR FILE HEADER
 * ============================================================================ */

typedef struct {
    uint32_t num_vectors;
    uint32_t dim;
    uint32_t reserved1;
    uint32_t reserved2;
} vector_file_header_t;

#endif /* MAGE_TYPES_H */
