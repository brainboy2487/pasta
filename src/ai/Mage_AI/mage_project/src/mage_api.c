/* mage_api.c - Refactored Middleman API implementations
 * Drop-in replacement for the original file with improved safety and performance.
 */
#define _GNU_SOURCE

#include "mage/api.h"
#include "mage/parser.h"
#include "mage/context.h"
#include "mage/contract.h"
#include "mage/subproc.h"
#include "mage/log.h"
#include "mage/plugin.h"
#include "mage/io.h"
#include "mage/mphf.h"
#include "mage/manifest.h"
#include "mage/generators.h"

#include <stdlib.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <limits.h>
#include <unistd.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <errno.h>
#include <stdarg.h> /* <-- required for va_start / va_end */

/* ---------- Configuration and small utilities ---------- */

/* Default vectors file path; can be changed via mage_api_set_vectors_path */
static char *g_vectors_path = NULL;
static const char *mage_api_default_vectors_path = "data/vectors.bin";

/* Safe strdup replacement */
static char* safe_strdup(const char* s) {
    if (!s) return NULL;
    size_t n = strlen(s) + 1;
    char *r = malloc(n);
    if (!r) return NULL;
    memcpy(r, s, n);
    return r;
}

/* Safe realloc wrapper that preserves original pointer on failure */
static void* safe_realloc(void* p, size_t new_size) {
    if (!p) return malloc(new_size);
    void* t = realloc(p, new_size);
    return t ? t : p;
}

/* Set custom vectors path at runtime. Optional convenience. */
int mage_api_set_vectors_path(const char* path) {
    if (!path) return MAGE_ERR_INVALID_ARG;
    char *dup = safe_strdup(path);
    if (!dup) return MAGE_ERR_GENERIC;
    if (g_vectors_path) free(g_vectors_path);
    g_vectors_path = dup;
    return 0;
}

/* Get current vectors path */
static const char* get_vectors_path(void) {
    return g_vectors_path ? g_vectors_path : mage_api_default_vectors_path;
}

/* ---------- Logging helpers ---------- */

/* Use vfprintf to stderr for variadic logging helper to avoid depending on mage_log internals */
static void log_error(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    fprintf(stderr, "\n");
    va_end(ap);
}

/* ---------- JSON escaping ---------- */

/* JSON escaper for small strings. Returns newly-allocated string or NULL. */
static char* escape_json_string(const char* s) {
    if (!s) return NULL;
    size_t len = strlen(s);
    /* worst-case: every char becomes \u00XX (6 chars) */
    size_t cap = len * 6 + 1;
    char* out = malloc(cap);
    if (!out) return NULL;
    size_t o = 0;
    for (const unsigned char* p = (const unsigned char*)s; *p; ++p) {
        if (o + 6 >= cap) {
            size_t newcap = cap * 2;
            char* t = realloc(out, newcap);
            if (!t) { free(out); return NULL; }
            out = t; cap = newcap;
        }
        unsigned char c = *p;
        switch (c) {
            case '"': out[o++] = '\\'; out[o++] = '"'; break;
            case '\\': out[o++] = '\\'; out[o++] = '\\'; break;
            case '\b': out[o++] = '\\'; out[o++] = 'b'; break;
            case '\f': out[o++] = '\\'; out[o++] = 'f'; break;
            case '\n': out[o++] = '\\'; out[o++] = 'n'; break;
            case '\r': out[o++] = '\\'; out[o++] = 'r'; break;
            case '\t': out[o++] = '\\'; out[o++] = 't'; break;
            default:
                if (c < 0x20) {
                    /* control char -> \u00XX */
                    char buf[7];
                    snprintf(buf, sizeof(buf), "\\u%04x", c);
                    size_t bl = strlen(buf);
                    memcpy(out + o, buf, bl); o += bl;
                } else {
                    out[o++] = c;
                }
        }
    }
    out[o] = '\0';
    return out;
}

/* Wrap generated text into JSON object {"generated": "..."}. Returns allocated string. */
static char* json_wrap_generated(const char* escaped_text) {
    if (!escaped_text) return NULL;
    size_t need = strlen(escaped_text) + 32;
    char* out = malloc(need);
    if (!out) return NULL;
    snprintf(out, need, "{\"generated\": \"%s\"}", escaped_text);
    return out;
}

/* ---------- Vector utilities ---------- */

/* Minimal deterministic vectorizer retained but tightened */
int mage_api_vectorize_text(const char* text, float* out_buf, uint32_t dim) {
    if (!out_buf || dim == 0) return MAGE_ERR_INVALID_ARG;
    uint32_t h = 1469598101u;
    if (text) {
        const unsigned char* s = (const unsigned char*)text;
        while (*s) { h = (h ^ *s++) * 16777619u; }
    }
    for (uint32_t i = 0; i < dim; ++i) {
        out_buf[i] = (float)((((h >> (i % 24)) & 0xFF) - 128) / 128.0);
    }
    return 0;
}

/* Cosine similarity */
static double cos_sim(const float* a, const float* b, uint32_t d) {
    double na = 0.0, nb = 0.0, dot = 0.0;
    for (uint32_t i = 0; i < d; ++i) {
        double va = (double)a[i];
        double vb = (double)b[i];
        dot += va * vb;
        na += va * va;
        nb += vb * vb;
    }
    if (na == 0.0 || nb == 0.0) return 0.0;
    return dot / (sqrt(na) * sqrt(nb));
}

/* ---------- Efficient top-K selection using min-heap ---------- */

typedef struct {
    double score;
    uint32_t idx;
} heap_node_t;

typedef struct {
    heap_node_t *nodes;
    uint32_t size;
    uint32_t capacity;
} min_heap_t;

static min_heap_t* heap_create(uint32_t capacity) {
    min_heap_t* h = malloc(sizeof(min_heap_t));
    if (!h) return NULL;
    h->nodes = malloc(sizeof(heap_node_t) * capacity);
    if (!h->nodes) { free(h); return NULL; }
    h->size = 0; h->capacity = capacity;
    return h;
}

static void heap_free(min_heap_t* h) {
    if (!h) return;
    free(h->nodes);
    free(h);
}

static void heap_swap(heap_node_t* a, heap_node_t* b) {
    heap_node_t t = *a; *a = *b; *b = t;
}

static void heap_sift_up(min_heap_t* h, uint32_t pos) {
    while (pos > 0) {
        uint32_t parent = (pos - 1) >> 1;
        if (h->nodes[parent].score <= h->nodes[pos].score) break;
        heap_swap(&h->nodes[parent], &h->nodes[pos]);
        pos = parent;
    }
}

static void heap_sift_down(min_heap_t* h, uint32_t pos) {
    uint32_t n = h->size;
    while (1) {
        uint32_t l = pos * 2 + 1;
        uint32_t r = l + 1;
        uint32_t smallest = pos;
        if (l < n && h->nodes[l].score < h->nodes[smallest].score) smallest = l;
        if (r < n && h->nodes[r].score < h->nodes[smallest].score) smallest = r;
        if (smallest == pos) break;
        heap_swap(&h->nodes[pos], &h->nodes[smallest]);
        pos = smallest;
    }
}

/* Push node; if capacity reached and new score > min, replace min */
static int heap_push(min_heap_t* h, double score, uint32_t idx) {
    if (h->size < h->capacity) {
        uint32_t pos = h->size++;
        h->nodes[pos].score = score;
        h->nodes[pos].idx = idx;
        heap_sift_up(h, pos);
        return 0;
    }
    /* if new score <= min, ignore */
    if (h->nodes[0].score >= score) return 0;
    /* replace root and sift down */
    h->nodes[0].score = score;
    h->nodes[0].idx = idx;
    heap_sift_down(h, 0);
    return 0;
}

/* Extract heap contents into arrays sorted descending by score */
static void heap_extract_sorted(min_heap_t* h, uint32_t *out_idxs, double *out_scores, uint32_t *out_k) {
    uint32_t k = h->size;
    /* simple selection: pop min repeatedly into temporary array then reverse */
    for (uint32_t i = 0; i < k; ++i) {
        out_idxs[i] = h->nodes[0].idx;
        out_scores[i] = h->nodes[0].score;
        /* replace root with last and sift down */
        h->nodes[0] = h->nodes[--h->size];
        heap_sift_down(h, 0);
    }
    /* reverse to descending order */
    for (uint32_t i = 0; i < k / 2; ++i) {
        uint32_t ti = out_idxs[i]; out_idxs[i] = out_idxs[k - 1 - i]; out_idxs[k - 1 - i] = ti;
        double td = out_scores[i]; out_scores[i] = out_scores[k - 1 - i]; out_scores[k - 1 - i] = td;
    }
    *out_k = k;
}

/* ---------- API functions ---------- */

int mage_api_init(void) {
    mage_log(MAGE_LOG_INFO, "mage_api: init");
    int n = mage_plugins_load();
    if (n > 0) mage_log(MAGE_LOG_INFO, "mage_api: loaded %d plugins", n);
    extern int mage_self_heal_start(void);
    (void)mage_self_heal_start();
    return 0;
}

void mage_api_shutdown(void) {
    mage_log(MAGE_LOG_INFO, "mage_api: shutdown");
    extern void mage_self_heal_stop(void);
    mage_self_heal_stop();
    mage_plugins_unload();
    if (g_vectors_path) { free(g_vectors_path); g_vectors_path = NULL; }
    mage_log_close();
}

int mage_api_print_help_all(void) {
    return mage_print_help_all();
}

int mage_api_train_from_file(const char* path) {
    if (!path) return MAGE_ERR_INVALID_ARG;
    const char *vout = "data/vocab/generated_vocab.jsonl";
    if (strstr(path, ".jsonl") || strstr(path, ".txt")) {
        int wrote = mage_api_build_vocab_from_file(path, vout, 1);
        if (wrote > 0) fprintf(stderr, "[mage_api] Built vocab (%d tokens) -> %s\n", wrote, vout);
        else fprintf(stderr, "[mage_api] Vocab build produced no tokens or failed\n");
    }
    int rc = mage_train_from_file(path);
    return (rc < 0) ? MAGE_ERR_GENERIC : rc;
}

char* mage_api_finalize_reply(const char* reply) {
    return mage_finalize_reply(reply);
}

/* Contract management wrappers */
int mage_api_contract_set_cache(size_t entries) {
    return contract_set_cache_capacity(entries);
}

void mage_api_contract_flush(void) {
    contract_flush_cache();
}

int mage_api_contract_precompute(const mage_slice_t* slices, uint32_t slice_idx, size_t row_start, size_t col_start, size_t block_size) {
    return contract_precompute_block(slices, slice_idx, row_start, col_start, block_size);
}

/* MMapped index wrappers */
mmapped_index_t* mage_api_mmapped_open(const char* path) {
    return io_mmapped_open(path);
}

int mage_api_mmapped_get(mmapped_index_t* idx, uint32_t i, float* out_buf) {
    return io_mmapped_get(idx, i, out_buf);
}

void mage_api_mmapped_close(mmapped_index_t* idx) {
    io_mmapped_close(idx);
}

uint32_t mage_api_mmapped_dim(mmapped_index_t* idx) {
    return io_mmapped_dim(idx);
}

/* Efficient retrieve: uses min-heap top-K selection */
char* mage_api_retrieve(const float* query, uint32_t dim, uint32_t k) {
    if (!query || dim == 0 || k == 0) return NULL;

    float* vecs = NULL;
    uint32_t n = 0, d = 0;
    const char *vpath = get_vectors_path();
    if (read_vectors_bin(vpath, &vecs, &n, &d) != 0) {
        /* no vectors available: return empty results json */
        char* res = malloc(32);
        if (!res) return NULL;
        strcpy(res, "{\"results\": []}");
        return res;
    }
    if (d != dim) {
        free(vecs);
        char* res = malloc(64);
        if (!res) return NULL;
        strcpy(res, "{\"error\": \"dim_mismatch\"}");
        return res;
    }

    uint32_t K = (k < n) ? k : n;
    min_heap_t* heap = heap_create(K);
    if (!heap) { free(vecs); return NULL; }

    /* compute scores and maintain min-heap of size K */
    for (uint32_t i = 0; i < n; ++i) {
        const float* v = vecs + (size_t)i * d;
        double s = cos_sim(query, v, d);
        heap_push(heap, s, i);
    }

    /* extract sorted results */
    uint32_t *idxs = malloc(sizeof(uint32_t) * K);
    double *scores = malloc(sizeof(double) * K);
    if (!idxs || !scores) { free(idxs); free(scores); heap_free(heap); free(vecs); return NULL; }
    uint32_t found = 0;
    heap_extract_sorted(heap, idxs, scores, &found);
    heap_free(heap);

    /* build JSON string */
    size_t cap = 1024;
    char* out = malloc(cap);
    if (!out) { free(idxs); free(scores); free(vecs); return NULL; }
    size_t len = snprintf(out, cap, "{\"results\": [");
    for (uint32_t j = 0; j < found; ++j) {
        if (j) { len += snprintf(out + len, cap - len, ", "); }
        /* ensure capacity */
        if (len + 128 > cap) {
            while (len + 128 > cap) cap *= 2;
            char* t = realloc(out, cap);
            if (!t) break;
            out = t;
        }
        len += snprintf(out + len, cap - len, "{\"id\": %u, \"score\": %.6f}", idxs[j], scores[j]);
    }
    len += snprintf(out + len, cap - len, "]}");

    free(idxs); free(scores); free(vecs);
    return out;
}

/* Parse wrapper */
char* mage_api_parse_prompt(const char* prompt) {
    if (!prompt) return NULL;
    return mage_parse_prompt(prompt);
}

/* High-level generate: parse prompt and run finalizer; return JSON string. */
char* mage_api_generate(const char* prompt) {
    if (!prompt) return NULL;
    char* parsed = mage_api_parse_prompt(prompt);
    if (!parsed) return NULL;

    extern char* warm_generate_from_vocab(const char* prompt, const char* vocab_path, int max_tokens);
    extern char* cold_generate_from_vocab(const char* prompt, const char* vocab_path, int max_tokens);
    const char *vocab_path = "data/vocab/generated_vocab.jsonl";

    char *warm = warm_generate_from_vocab(parsed, vocab_path, 8);
    char *cold = cold_generate_from_vocab(parsed, vocab_path, 4);
    char* finalized = mage_api_finalize_reply(parsed);
    free(parsed);

    /* Merge outputs */
    size_t needbuf = 256;
    if (finalized) needbuf += strlen(finalized);
    if (warm) needbuf += strlen(warm);
    if (cold) needbuf += strlen(cold);
    char *merged = malloc(needbuf);
    if (!merged) { if (finalized) free(finalized); if (warm) free(warm); if (cold) free(cold); return NULL; }
    merged[0] = '\0';
    if (finalized) { strcat(merged, finalized); }
    if (warm) { if (merged[0]) strcat(merged, " "); strcat(merged, warm); }
    if (cold) { if (merged[0]) strcat(merged, " "); strcat(merged, cold); }
    if (finalized) { free(finalized); finalized = NULL; }
    if (warm) { free(warm); warm = NULL; }
    if (cold) { free(cold); cold = NULL; }

    /* Run coherence checker */
    char *checked = NULL;
    {
        const char *const argvv[] = {"python3", "python/coherence_checker.py", NULL};
        int exitcode = -1;
        if (subproc_run_capture_stdin(argvv, merged, 10000, &checked, &exitcode) != 0) {
            if (checked) { free(checked); checked = NULL; }
        }
    }

    char *use = NULL;
    if (checked && checked[0]) use = checked; else use = safe_strdup(merged);
    free(merged);
    if (!use) return NULL;

    char* esc = escape_json_string(use);
    free(use);
    if (!esc) return NULL;

    char* out = json_wrap_generated(esc);
    free(esc);
    return out;
}

/* Context and manifest wrappers */
int mage_api_context_load_from_jsonl(const char* path) {
    if (!path) return MAGE_ERR_INVALID_ARG;
    int rc = mage_context_load_from_jsonl(path);
    return (rc < 0) ? MAGE_ERR_GENERIC : rc;
}

int mage_api_mphf_probe(const char* path) {
    if (!path) return 0;
    extern int mphf_loader_probe(const char* p);
    return mphf_loader_probe(path);
}

manifest_t* mage_api_manifest_load(const char* path) {
    return manifest_load(path);
}

const manifest_entry_t* mage_api_manifest_get(const manifest_t* m, uint32_t idx) {
    return manifest_get(m, idx);
}

void mage_api_manifest_free(manifest_t* m) {
    manifest_free(m);
}
