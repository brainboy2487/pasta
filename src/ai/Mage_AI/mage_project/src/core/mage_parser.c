/* mage_parser.c - Refactored parser, vocab loader, trainer, and coherence checker
 *
 * Drop-in replacement for the original mage_parser.c with:
 *  - Safer memory handling and bounds checks
 *  - Threaded coherence checker queue with clean startup/shutdown
 *  - Robust JSONL vocab parsing and validation
 *  - Deterministic, testable training pipeline that writes generated vocab
 *  - Public API functions preserved:
 *      int mage_train_from_file(const char* path);
 *      char* mage_finalize_reply(const char* reply);
 *      char* mage_parse_prompt(const char* prompt);
 *      int mage_api_build_vocab_from_file(const char* in, const char* out, int min_count);
 *      int mage_api_tokenize(const char* text, char*** out_tokens, size_t* out_count); (wrapper)
 *      void mage_api_free_tokens(char** toks, size_t count);
 *
 * Notes:
 *  - This file assumes the existence of helper modules declared in headers:
 *      mage/tokenizer.h  (mage_tokenize, mage_free_tokens, mage_normalize)
 *      mage/hash.h       (hash_murmur64)
 *      mage/subproc.h    (subproc_run_capture_stdin)
 *    Keep those available in the build.
 */

#define _POSIX_C_SOURCE 200809L
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include "mage/parser.h"
#include "mage/tokenizer.h"
#include "mage/hash.h"
#include "mage/subproc.h"

#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include <stdint.h>
#include <errno.h>
#include <glob.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <pthread.h>
#include <limits.h>
#include <stdarg.h>

/* ----------------------------- Configuration ------------------------------ */

#define VOCAB_LINE_MAX 16384
#define GENERATED_VOCAB_PATH "data/vocab/generated_vocab.jsonl"
#define COHERENCE_TIMEOUT_MS 10000
#define COHERENCE_FALLBACK_MSG "Sorry, I can't produce a coherent reply right now."

/* ----------------------------- Data structures --------------------------- */

typedef struct {
    char *key;
    char **responses;
    size_t resp_count;
} vocab_entry_t;

/* Global vocabulary (thread-safe via mutex) */
static vocab_entry_t *VOCAB = NULL;
static size_t VOCAB_COUNT = 0;
static int VOCAB_LOADED = 0;
static pthread_mutex_t VOCAB_MUTEX = PTHREAD_MUTEX_INITIALIZER;

/* ----------------------------- JSON helpers ------------------------------ */

/* Parse a JSON string starting at *pp. Advances *pp past the closing quote.
 * Returns a newly-allocated C string or NULL on error.
 */
static char* parse_json_string(const char **pp) {
    if (!pp || !*pp) return NULL;
    const char *p = *pp;
    while (*p && isspace((unsigned char)*p)) ++p;
    if (*p != '"') return NULL;
    ++p;
    size_t cap = 64, len = 0;
    char *out = malloc(cap);
    if (!out) return NULL;
    while (*p) {
        char c = *p++;
        if (c == '"') break;
        if (c == '\\' && *p) {
            char esc = *p++;
            switch (esc) {
                case 'n': c = '\n'; break;
                case 'r': c = '\r'; break;
                case 't': c = '\t'; break;
                case '"': c = '"'; break;
                case '\\': c = '\\'; break;
                case 'u': {
                    /* simple \uXXXX handling for ASCII-range control codes */
                    unsigned int code = 0;
                    int i;
                    for (i = 0; i < 4 && isxdigit((unsigned char)*p); ++i) {
                        char ch = *p++;
                        code <<= 4;
                        if (ch >= '0' && ch <= '9') code |= (ch - '0');
                        else if (ch >= 'a' && ch <= 'f') code |= (10 + ch - 'a');
                        else if (ch >= 'A' && ch <= 'F') code |= (10 + ch - 'A');
                    }
                    if (i == 4 && code < 0x80) c = (char)code;
                    else c = '?';
                    break;
                }
                default: c = esc; break;
            }
        }
        if (len + 1 >= cap) {
            cap *= 2;
            char *t = realloc(out, cap);
            if (!t) { free(out); return NULL; }
            out = t;
        }
        out[len++] = c;
    }
    out[len] = '\0';
    *pp = p;
    return out;
}

/* Skip whitespace and commas */
static void skip_ws_commas(const char **pp) {
    const char *p = *pp;
    while (*p && (isspace((unsigned char)*p) || *p == ',')) ++p;
    *pp = p;
}

/* Parse JSON array of strings. Returns newly-allocated array and sets count. */
static char** parse_json_array_of_strings(const char **pp, size_t *count_out) {
    if (!pp || !count_out) return NULL;
    *count_out = 0;
    const char *p = *pp;
    while (*p && isspace((unsigned char)*p)) ++p;
    if (*p != '[') return NULL;
    ++p;
    skip_ws_commas(&p);
    char **arr = NULL;
    size_t cnt = 0;
    while (*p && *p != ']') {
        char *s = parse_json_string(&p);
        if (!s) {
            for (size_t i = 0; i < cnt; ++i) free(arr[i]);
            free(arr);
            return NULL;
        }
        char **t = realloc(arr, (cnt + 1) * sizeof(char*));
        if (!t) { free(s); for (size_t i = 0; i < cnt; ++i) free(arr[i]); free(arr); return NULL; }
        arr = t;
        arr[cnt++] = s;
        skip_ws_commas(&p);
    }
    if (*p == ']') ++p;
    *pp = p;
    *count_out = cnt;
    return arr;
}

/* JSON escape writer for FILE */
static void json_fprintf_escaped(FILE *out, const char *s) {
    if (!out || !s) return;
    const unsigned char *p = (const unsigned char*)s;
    while (*p) {
        unsigned char c = *p++;
        switch (c) {
            case '"': fputs("\\\"", out); break;
            case '\\': fputs("\\\\", out); break;
            case '\b': fputs("\\b", out); break;
            case '\f': fputs("\\f", out); break;
            case '\n': fputs("\\n", out); break;
            case '\r': fputs("\\r", out); break;
            case '\t': fputs("\\t", out); break;
            default:
                if (c < 0x20) fprintf(out, "\\u%04x", c);
                else fputc(c, out);
        }
    }
}

/* ----------------------------- Vocab management --------------------------- */

/* Add a vocab entry (takes ownership of key and responses). Thread-safe. */
static int vocab_add_entry_locked(char *key, char **responses, size_t resp_count) {
    if (!key || !responses || resp_count == 0) {
        /* free if partially provided */
        if (key) free(key);
        if (responses) {
            for (size_t i = 0; i < resp_count; ++i) free(responses[i]);
            free(responses);
        }
        return -1;
    }
    vocab_entry_t *n = realloc(VOCAB, (VOCAB_COUNT + 1) * sizeof(vocab_entry_t));
    if (!n) {
        free(key);
        for (size_t i = 0; i < resp_count; ++i) free(responses[i]);
        free(responses);
        return -1;
    }
    VOCAB = n;
    VOCAB[VOCAB_COUNT].key = key;
    VOCAB[VOCAB_COUNT].responses = responses;
    VOCAB[VOCAB_COUNT].resp_count = resp_count;
    VOCAB_COUNT++;
    return 0;
}

/* Clear the in-memory VOCAB (thread-safe) */
static void vocab_clear_locked(void) {
    for (size_t i = 0; i < VOCAB_COUNT; ++i) {
        free(VOCAB[i].key);
        for (size_t j = 0; j < VOCAB[i].resp_count; ++j) free(VOCAB[i].responses[j]);
        free(VOCAB[i].responses);
    }
    free(VOCAB);
    VOCAB = NULL;
    VOCAB_COUNT = 0;
    VOCAB_LOADED = 0;
}

/* Load a single JSONL vocab file. Returns number of entries added or -1 on error. */
static int load_vocab_from_file_locked(const char *path) {
    if (!path) return -1;
    FILE *f = fopen(path, "r");
    if (!f) return -1;
    char *line = NULL;
    size_t cap = 0;
    ssize_t nread;
    int added = 0;
    while ((nread = getline(&line, &cap, f)) != -1) {
        /* trim newline */
        while (nread > 0 && (line[nread-1] == '\n' || line[nread-1] == '\r')) line[--nread] = '\0';
        if (nread == 0) continue;
        /* parse object fields */
        const char *p = line;
        /* minimal parse: find "key" and "responses" */
        char *key = NULL;
        char **responses = NULL;
        size_t resp_count = 0;
        while (*p) {
            while (*p && isspace((unsigned char)*p)) ++p;
            if (*p == '{') { ++p; continue; }
            if (*p == '}') break;
            char *field = parse_json_string(&p);
            if (!field) break;
            while (*p && isspace((unsigned char)*p)) ++p;
            if (*p != ':') { free(field); break; }
            ++p;
            while (*p && isspace((unsigned char)*p)) ++p;
            if (strcmp(field, "key") == 0) {
                key = parse_json_string(&p);
            } else if (strcmp(field, "responses") == 0) {
                responses = parse_json_array_of_strings(&p, &resp_count);
            } else {
                /* skip unknown: if string, parse and free; if array, parse and free; else skip token */
                if (*p == '"') { char *tmp = parse_json_string(&p); if (tmp) free(tmp); }
                else if (*p == '[') {
                    size_t dummy = 0;
                    char **tmp = parse_json_array_of_strings(&p, &dummy);
                    if (tmp) { for (size_t i = 0; i < dummy; ++i) free(tmp[i]); free(tmp); }
                } else {
                    while (*p && *p != ',' && *p != '}') ++p;
                }
            }
            free(field);
            while (*p && (isspace((unsigned char)*p) || *p == ',')) ++p;
        }
        if (key && responses && resp_count > 0) {
            /* normalize key using tokenizer's normalize if available */
            char *nkey = mage_normalize ? mage_normalize(key) : key;
            if (nkey != key) free(key);
            if (vocab_add_entry_locked(nkey, responses, resp_count) == 0) added++;
            else {
                /* vocab_add_entry_locked frees responses on failure */
            }
        } else {
            if (key) free(key);
            if (responses) {
                for (size_t i = 0; i < resp_count; ++i) free(responses[i]);
                free(responses);
            }
        }
    }
    free(line);
    fclose(f);
    return added;
}

/* Load all vocab files (data/vocab_*.jsonl and generated vocab). Thread-safe. */
static int load_vocab_all_locked(void) {
    if (VOCAB_LOADED) return 0;
    VOCAB_LOADED = 1;
    glob_t g;
    int total = 0;
    if (glob("data/vocab_*.jsonl", 0, NULL, &g) == 0) {
        for (size_t i = 0; i < g.gl_pathc; ++i) {
            int r = load_vocab_from_file_locked(g.gl_pathv[i]);
            if (r > 0) total += r;
        }
        globfree(&g);
    }
    /* generated vocab path (compat) */
    int r = load_vocab_from_file_locked(GENERATED_VOCAB_PATH);
    if (r > 0) total += r;
    return total;
}

/* Public API: build vocab from input file and write to out_path (simple frequency-based).
 * Returns number of tokens written or negative on error.
 */
int mage_api_build_vocab_from_file(const char* in_path, const char* out_path, int min_count) {
    if (!in_path || !out_path) return -1;
    FILE *f = fopen(in_path, "r");
    if (!f) return -1;
    /* Simple frequency map using chained hash table (string keys) */
    typedef struct kv { char *k; size_t count; struct kv *next; } kv_t;
    const size_t HT = 65536;
    kv_t **table = calloc(HT, sizeof(kv_t*));
    if (!table) { fclose(f); return -1; }
    char *line = NULL;
    size_t cap = 0;
    ssize_t nread;
    while ((nread = getline(&line, &cap, f)) != -1) {
        /* tokenize line using mage_tokenize if available */
        char **toks = NULL; size_t tcount = 0;
        if (mage_tokenize && mage_tokenize(line, &toks, &tcount) == 0 && tcount > 0) {
            for (size_t i = 0; i < tcount; ++i) {
                const char *tok = toks[i];
                uint64_t h = hash_murmur64(tok, strlen(tok), 0);
                size_t idx = (size_t)(h % HT);
                kv_t *b = table[idx];
                while (b) {
                    if (strcmp(b->k, tok) == 0) break;
                    b = b->next;
                }
                if (b) b->count++;
                else {
                    kv_t *n = malloc(sizeof(kv_t));
                    n->k = strdup(tok);
                    n->count = 1;
                    n->next = table[idx];
                    table[idx] = n;
                }
            }
            mage_free_tokens(toks, tcount);
        }
    }
    free(line);
    fclose(f);

    /* Write out entries with count >= min_count */
    FILE *out = fopen(out_path, "w");
    if (!out) {
        for (size_t i = 0; i < HT; ++i) {
            kv_t *b = table[i];
            while (b) { kv_t *nx = b->next; free(b->k); free(b); b = nx; }
        }
        free(table);
        return -1;
    }
    int written = 0;
    for (size_t i = 0; i < HT; ++i) {
        kv_t *b = table[i];
        while (b) {
            if ((int)b->count >= min_count) {
                fprintf(out, "{\"token\":\"");
                json_fprintf_escaped(out, b->k);
                fprintf(out, "\",\"count\":%zu}\n", b->count);
                written++;
            }
            kv_t *nx = b->next;
            free(b->k);
            free(b);
            b = nx;
        }
    }
    free(table);
    fclose(out);
    return written;
}

/* ------------------------- Coherence checker worker ----------------------- */

/* Job structure for background coherence checks */
typedef struct coherence_job {
    char *input;            /* input string (owned) */
    char *output;           /* output string (owned) */
    int done;
    pthread_mutex_t m;
    pthread_cond_t c;
    struct coherence_job *next;
} coherence_job_t;

static coherence_job_t *cq_head = NULL;
static coherence_job_t *cq_tail = NULL;
static pthread_mutex_t cq_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t cq_cond = PTHREAD_COND_INITIALIZER;
static pthread_t cq_thread;
static int cq_running = 0;
static int cq_stop = 0;

/* Create and free job */
static coherence_job_t* coherence_job_new(char *input) {
    coherence_job_t *j = malloc(sizeof(*j));
    if (!j) return NULL;
    j->input = input;
    j->output = NULL;
    j->done = 0;
    pthread_mutex_init(&j->m, NULL);
    pthread_cond_init(&j->c, NULL);
    j->next = NULL;
    return j;
}
static void coherence_job_free(coherence_job_t *j) {
    if (!j) return;
    pthread_mutex_destroy(&j->m);
    pthread_cond_destroy(&j->c);
    free(j);
}

/* Enqueue and dequeue */
static void coherence_enqueue(coherence_job_t *j) {
    pthread_mutex_lock(&cq_mutex);
    j->next = NULL;
    if (!cq_tail) cq_head = cq_tail = j;
    else { cq_tail->next = j; cq_tail = j; }
    pthread_cond_signal(&cq_cond);
    pthread_mutex_unlock(&cq_mutex);
}
static coherence_job_t* coherence_dequeue(void) {
    pthread_mutex_lock(&cq_mutex);
    coherence_job_t *j = cq_head;
    if (j) {
        cq_head = cq_head->next;
        if (!cq_head) cq_tail = NULL;
    }
    pthread_mutex_unlock(&cq_mutex);
    return j;
}

/* Worker thread: runs python coherence_checker.py on job->input and stores output */
static void* coherence_worker(void *arg) {
    (void)arg;
    while (1) {
        pthread_mutex_lock(&cq_mutex);
        while (!cq_head && !cq_stop) pthread_cond_wait(&cq_cond, &cq_mutex);
        if (cq_stop && !cq_head) { pthread_mutex_unlock(&cq_mutex); break; }
        coherence_job_t *job = coherence_dequeue();
        pthread_mutex_unlock(&cq_mutex);
        if (!job) continue;
        /* run checker */
        char *checked = NULL;
        int status = -1;
        const char *const argvv[] = {"python3", "python/coherence_checker.py", NULL};
        if (subproc_run_capture_stdin(argvv, job->input, COHERENCE_TIMEOUT_MS, &checked, &status) != 0) {
            if (checked) { free(checked); checked = NULL; }
        }
        pthread_mutex_lock(&job->m);
        job->output = checked;
        job->done = 1;
        pthread_cond_signal(&job->c);
        pthread_mutex_unlock(&job->m);
    }
    return NULL;
}

/* Start/stop worker */
static int coherence_start_worker(void) {
    pthread_mutex_lock(&cq_mutex);
    if (cq_running) { pthread_mutex_unlock(&cq_mutex); return 0; }
    cq_stop = 0;
    cq_head = cq_tail = NULL;
    if (pthread_create(&cq_thread, NULL, coherence_worker, NULL) != 0) {
        pthread_mutex_unlock(&cq_mutex);
        return -1;
    }
    cq_running = 1;
    pthread_mutex_unlock(&cq_mutex);
    return 0;
}

static void coherence_stop_worker(void) {
    pthread_mutex_lock(&cq_mutex);
    if (!cq_running) { pthread_mutex_unlock(&cq_mutex); return; }
    cq_stop = 1;
    pthread_cond_signal(&cq_cond);
    pthread_mutex_unlock(&cq_mutex);
    pthread_join(cq_thread, NULL);
    cq_running = 0;
    /* Drain remaining jobs and signal them with synchronous fallback */
    coherence_job_t *j;
    while ((j = coherence_dequeue()) != NULL) {
        pthread_mutex_lock(&j->m);
        if (!j->done) {
            /* synchronous fallback */
            char *checked = NULL;
            int status = -1;
            const char *const argvv[] = {"python3", "python/coherence_checker.py", NULL};
            if (subproc_run_capture_stdin(argvv, j->input, COHERENCE_TIMEOUT_MS, &checked, &status) != 0) {
                if (checked) { free(checked); checked = NULL; }
            }
            j->output = checked;
            j->done = 1;
            pthread_cond_signal(&j->c);
        }
        pthread_mutex_unlock(&j->m);
        free(j->input);
        coherence_job_free(j);
    }
}

/* --------------------------- Public API: finalize ------------------------- */

/* Run the coherence checker synchronously (via subproc) with fallbacks.
 * Returns a newly-allocated string (caller must free) or NULL on error.
 */
char* mage_finalize_reply(const char *reply) {
    if (!reply) return NULL;
    size_t len = strlen(reply);
    /* short single-token replies: return copy directly to avoid external checker */
    if (len > 0 && len <= 64 && strchr(reply, ' ') == NULL) {
        char *cp = malloc(len + 1);
        if (cp) strcpy(cp, reply);
        return cp;
    }
    /* Try to run checker synchronously */
    char *out = NULL;
    int status = -1;
    const char *const argvv[] = {"python3", "python/coherence_checker.py", NULL};
    if (subproc_run_capture_stdin(argvv, reply, COHERENCE_TIMEOUT_MS, &out, &status) != 0) {
        if (out) { free(out); out = NULL; }
        /* fallback: return copy of original */
        char *cp = malloc(len + 1);
        if (cp) strcpy(cp, reply);
        return cp;
    }
    if (!out || strlen(out) == 0) {
        if (out) free(out);
        char *cp = malloc(len + 1);
        if (cp) strcpy(cp, reply);
        return cp;
    }
    /* trim trailing newlines */
    size_t olen = strlen(out);
    while (olen > 0 && (out[olen-1] == '\n' || out[olen-1] == '\r')) out[--olen] = '\0';
    char *ret = malloc(olen + 1);
    if (ret) strcpy(ret, out);
    free(out);
    if (ret) return ret;
    /* allocation failed: fallback to original */
    char *cp = malloc(len + 1);
    if (cp) strcpy(cp, reply);
    return cp;
}

/* --------------------------- Public API: parse ---------------------------- */

/* Very small prompt parser: normalizes and looks up in VOCAB, falls back to canned DICT.
 * Returns newly-allocated string (caller must free) or NULL.
 */
char* mage_parse_prompt(const char *prompt) {
    if (!prompt) return NULL;
    /* normalize prompt */
    char *norm = mage_normalize ? mage_normalize(prompt) : strdup(prompt);
    if (!norm) return NULL;

    /* ensure vocab loaded */
    pthread_mutex_lock(&VOCAB_MUTEX);
    if (!VOCAB_LOADED) load_vocab_all_locked();
    /* search VOCAB for exact match */
    for (size_t i = 0; i < VOCAB_COUNT; ++i) {
        if (VOCAB[i].key && strcmp(VOCAB[i].key, norm) == 0) {
            /* pick first response deterministically */
            char *resp = NULL;
            if (VOCAB[i].resp_count > 0 && VOCAB[i].responses[0]) resp = strdup(VOCAB[i].responses[0]);
            pthread_mutex_unlock(&VOCAB_MUTEX);
            free(norm);
            return resp;
        }
    }
    pthread_mutex_unlock(&VOCAB_MUTEX);

    /* fallback to small in-code dictionary */
    /* simple canned responses */
    struct { const char *k; const char *v; } DICT[] = {
        {"hello", "Hello! I am Mage. How can I help you today?"},
        {"hi", "Hi — I'm Mage, a minimal interactive assistant."},
        {"hey", "Hey there — tell me a prompt and I'll respond."},
        {"bye", "Goodbye."},
        {"thanks", "You're welcome!"},
        {"help", "Type a short command or 'help -all' for more options."},
        {"ping", "pong"},
        {"version", "Mage base system v0.1"},
        {NULL, NULL}
    };
    for (int i = 0; DICT[i].k; ++i) {
        if (strcmp(DICT[i].k, norm) == 0) {
            char *r = strdup(DICT[i].v);
            free(norm);
            return r;
        }
    }

    /* last resort: return a generic reply */
    char *generic = strdup("I'm not sure how to respond to that. Try a different prompt.");
    free(norm);
    return generic;
}

/* --------------------------- Public API: training ------------------------- */

/* Helper: simple tokenizer-based trainer that builds next-word buckets and writes generated vocab.
 * Returns number of generated entries added or negative on error.
 */
int mage_train_from_file(const char *path) {
    if (!path) return -1;
    FILE *f = fopen(path, "r");
    if (!f) return -1;

    const size_t NB = 4096;
    typedef struct next_item { char *word; size_t count; } next_item_t;
    typedef struct word_bucket { char *token; next_item_t *nexts; size_t n_nexts; struct word_bucket *next; } word_bucket_t;

    word_bucket_t **table = calloc(NB, sizeof(word_bucket_t*));
    if (!table) { fclose(f); return -1; }

    char *line = NULL;
    size_t cap = 0;
    ssize_t nread;
    char prev_token[512] = "";
    int have_prev = 0;

    while ((nread = getline(&line, &cap, f)) != -1) {
        /* trim newline */
        while (nread > 0 && (line[nread-1] == '\n' || line[nread-1] == '\r')) line[--nread] = '\0';
        if (nread == 0) continue;
        char **toks = NULL; size_t tcount = 0;
        if (mage_tokenize && mage_tokenize(line, &toks, &tcount) == 0 && tcount > 0) {
            for (size_t ti = 0; ti < tcount; ++ti) {
                const char *tok = toks[ti];
                if (have_prev) {
                    uint64_t h = hash_murmur64(prev_token, strlen(prev_token), 0);
                    size_t idx = (size_t)(h % NB);
                    word_bucket_t *b = table[idx];
                    while (b) {
                        if (strcmp(b->token, prev_token) == 0) break;
                        b = b->next;
                    }
                    if (!b) {
                        b = malloc(sizeof(word_bucket_t));
                        b->token = strdup(prev_token);
                        b->nexts = NULL;
                        b->n_nexts = 0;
                        b->next = table[idx];
                        table[idx] = b;
                    }
                    /* increment next count */
                    size_t found = 0;
                    for (size_t j = 0; j < b->n_nexts; ++j) {
                        if (strcmp(b->nexts[j].word, tok) == 0) { b->nexts[j].count++; found = 1; break; }
                    }
                    if (!found) {
                        next_item_t *t = realloc(b->nexts, (b->n_nexts + 1) * sizeof(next_item_t));
                        if (!t) continue;
                        b->nexts = t;
                        b->nexts[b->n_nexts].word = strdup(tok);
                        b->nexts[b->n_nexts].count = 1;
                        b->n_nexts++;
                    }
                }
                strncpy(prev_token, tok, sizeof(prev_token)-1);
                prev_token[sizeof(prev_token)-1] = '\0';
                have_prev = 1;
            }
            mage_free_tokens(toks, tcount);
        }
    }
    free(line);
    fclose(f);

    /* For each bucket, pick top N nexts, run coherence checks, and add to VOCAB */
    size_t added = 0;
    /* start coherence worker */
    coherence_start_worker();

    for (size_t i = 0; i < NB; ++i) {
        word_bucket_t *b = table[i];
        while (b) {
            if (b->n_nexts > 0) {
                /* select top 5 by count */
                size_t take = b->n_nexts < 5 ? b->n_nexts : 5;
                /* create index array */
                size_t *idxs = malloc(b->n_nexts * sizeof(size_t));
                if (!idxs) { b = b->next; continue; }
                for (size_t j = 0; j < b->n_nexts; ++j) idxs[j] = j;
                for (size_t a = 0; a < take; ++a) {
                    size_t best = a;
                    for (size_t c = a+1; c < b->n_nexts; ++c) {
                        if (b->nexts[idxs[c]].count > b->nexts[idxs[best]].count) best = c;
                    }
                    size_t tmp = idxs[a]; idxs[a] = idxs[best]; idxs[best] = tmp;
                }
                /* prepare jobs */
                char **responses = NULL;
                size_t resp_count = 0;
                coherence_job_t **jobs = malloc(take * sizeof(coherence_job_t*));
                size_t nj = 0;
                for (size_t r = 0; r < take; ++r) {
                    const char *cand = b->nexts[idxs[r]].word;
                    char *copy = strdup(cand);
                    if (!copy) continue;
                    if (cq_running) {
                        coherence_job_t *job = coherence_job_new(copy);
                        if (!job) { free(copy); continue; }
                        jobs[nj++] = job;
                        coherence_enqueue(job);
                    } else {
                        /* synchronous fallback */
                        char *checked = mage_finalize_reply(copy);
                        if (checked && checked[0] != '\0' && strcmp(checked, COHERENCE_FALLBACK_MSG) != 0) {
                            char **t = realloc(responses, (resp_count + 1) * sizeof(char*));
                            responses = t;
                            responses[resp_count++] = checked;
                        } else {
                            if (checked) free(checked);
                        }
                        free(copy);
                    }
                }
                if (cq_running) {
                    for (size_t jj = 0; jj < nj; ++jj) {
                        coherence_job_t *job = jobs[jj];
                        pthread_mutex_lock(&job->m);
                        while (!job->done) pthread_cond_wait(&job->c, &job->m);
                        char *checked = job->output;
                        pthread_mutex_unlock(&job->m);
                        if (checked && checked[0] != '\0' && strcmp(checked, COHERENCE_FALLBACK_MSG) != 0) {
                            char **t = realloc(responses, (resp_count + 1) * sizeof(char*));
                            responses = t;
                            responses[resp_count++] = checked;
                        } else {
                            if (checked) free(checked);
                        }
                        free(job->input);
                        coherence_job_free(job);
                    }
                }
                free(jobs);
                free(idxs);

                /* Add to VOCAB if we have responses */
                if (resp_count > 0) {
                    char *nkey = mage_normalize ? mage_normalize(b->token) : strdup(b->token);
                    if (nkey) {
                        pthread_mutex_lock(&VOCAB_MUTEX);
                        if (vocab_add_entry_locked(nkey, responses, resp_count) == 0) added++;
                        else {
                            /* on failure, free responses */
                            for (size_t rr = 0; rr < resp_count; ++rr) free(responses[rr]);
                            free(responses);
                            free(nkey);
                        }
                        pthread_mutex_unlock(&VOCAB_MUTEX);
                    } else {
                        for (size_t rr = 0; rr < resp_count; ++rr) free(responses[rr]);
                        free(responses);
                    }
                } else {
                    /* free nothing: responses==NULL */
                }
            }
            b = b->next;
        }
    }

    /* write generated vocab file */
    FILE *out = fopen(GENERATED_VOCAB_PATH, "w");
    if (out) {
        pthread_mutex_lock(&VOCAB_MUTEX);
        for (size_t i = 0; i < VOCAB_COUNT; ++i) {
            vocab_entry_t *ve = &VOCAB[i];
            if (!ve->key || ve->resp_count == 0) continue;
            fprintf(out, "{\"id\":\"%016llx\",\"key\":\"", (unsigned long long)hash_murmur64(ve->key, strlen(ve->key), 0));
            json_fprintf_escaped(out, ve->key);
            fprintf(out, "\",\"type\":\"other\",\"responses\":[");
            for (size_t r = 0; r < ve->resp_count; ++r) {
                if (r) fprintf(out, ",");
                fprintf(out, "\"");
                json_fprintf_escaped(out, ve->responses[r]);
                fprintf(out, "\"");
            }
            fprintf(out, "]}\n");
        }
        pthread_mutex_unlock(&VOCAB_MUTEX);
        fclose(out);
    }

    /* stop worker and cleanup buckets */
    coherence_stop_worker();

    for (size_t i = 0; i < NB; ++i) {
        word_bucket_t *b = table[i];
        while (b) {
            word_bucket_t *nx = b->next;
            for (size_t j = 0; j < b->n_nexts; ++j) free(b->nexts[j].word);
            free(b->nexts);
            free(b->token);
            free(b);
            b = nx;
        }
    }
    free(table);
    return (int)added;
}

/* --------------------------- Tokenizer wrappers --------------------------- */

/* Expose tokenizer functions under consistent names expected by other modules */
int mage_api_tokenize(const char *text, char ***out_tokens, size_t *out_count) {
    if (!text || !out_tokens || !out_count) return -1;
    if (!mage_tokenize) return -1;
    return mage_tokenize(text, out_tokens, out_count);
}
void mage_api_free_tokens(char **toks, size_t count) {
    if (!toks) return;
    if (mage_free_tokens) { mage_free_tokens(toks, count); return; }
    for (size_t i = 0; i < count; ++i) free(toks[i]);
    free(toks);
}

/* --------------------------- Initialization / cleanup --------------------- */

/* Public helper to explicitly load vocab (thread-safe). Returns number added or negative on error. */
int mage_parser_load_vocab(void) {
    pthread_mutex_lock(&VOCAB_MUTEX);
    int r = load_vocab_all_locked();
    pthread_mutex_unlock(&VOCAB_MUTEX);
    return r;
}

/* Public helper to clear vocab from memory */
void mage_parser_clear_vocab(void) {
    pthread_mutex_lock(&VOCAB_MUTEX);
    vocab_clear_locked();
    pthread_mutex_unlock(&VOCAB_MUTEX);
}

/* ------------------------------- End of file ----------------------------- */
