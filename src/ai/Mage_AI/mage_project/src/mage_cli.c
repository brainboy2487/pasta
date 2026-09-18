/* mage_cli.c - Refactored, robust, and feature-complete CLI for Mage
 *
 * Drop-in replacement for the original mage_cli.c.
 *
 * Fixes:
 *  - Declares read_runtime_overrides prototype before use to avoid implicit declaration.
 *  - Removes unused helper to eliminate -Wunused-function warning.
 *  - Keeps behavior and commands compatible with existing Mage API.
 *
 * Build: replace src/mage_cli.c with this file and compile as before.
 */

#define _POSIX_C_SOURCE 200809L
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <libgen.h>
#include <unistd.h>
#include <errno.h>
#include <ctype.h>
#include <signal.h>
#include "mage/api.h"
#include "mage/subproc.h"
#include "mage/log.h"
#include "jsmn.h"

#define LINEBUF_SZ 4096
#define CLI_HISTORY_MAX 256

/* Redirect legacy system() usage to supervised runner with timeout */
#undef system
#define system(cmd) subproc_run_shell((cmd), 120000)

/* In-memory history */
static char *cli_history[CLI_HISTORY_MAX];
static size_t cli_history_count = 0;
static int verbose_mode = 0;

/* Forward declarations */
static void add_history(const char *line);
static void free_history(void);
static void interactive_mode(void);
static void handle_command_line(const char *line);
static void print_help_brief(void);
static void ensure_repo_root(int argc, char **argv);
static void handle_sigint(int sig);
/* Prototype added to avoid implicit declaration error */
static int read_runtime_overrides(unsigned long long *out_k, unsigned long long *out_seed);

/* Utility: trim leading/trailing whitespace in-place */
static void trim_inplace(char *s) {
    if (!s) return;
    char *p = s;
    while (isspace((unsigned char)*p)) ++p;
    if (p != s) memmove(s, p, strlen(p) + 1);
    size_t len = strlen(s);
    while (len > 0 && isspace((unsigned char)s[len - 1])) s[--len] = '\0';
}

/* Add to history (keeps most recent CLI_HISTORY_MAX entries) */
static void add_history(const char *line) {
    if (!line || !*line) return;
    if (cli_history_count > 0 && strcmp(cli_history[cli_history_count - 1], line) == 0) return;
    if (cli_history_count >= CLI_HISTORY_MAX) {
        free(cli_history[0]);
        memmove(cli_history, cli_history + 1, (CLI_HISTORY_MAX - 1) * sizeof(char*));
        cli_history_count = CLI_HISTORY_MAX - 1;
    }
    cli_history[cli_history_count++] = strdup(line);
}

/* Free history on exit */
static void free_history(void) {
    for (size_t i = 0; i < cli_history_count; ++i) free(cli_history[i]);
    cli_history_count = 0;
}

/* Signal handler for Ctrl-C */
static void handle_sigint(int sig) {
    (void)sig;
    fprintf(stderr, "\nInterrupted. Type 'exit' or Ctrl-D to quit.\n> ");
    fflush(stderr);
}

/* Ensure working directory is repo root when launched from bin/ */
static void ensure_repo_root(int argc, char **argv) {
    if (argc > 0 && argv[0]) {
        char real[PATH_MAX];
        if (realpath(argv[0], real)) {
            char *d = strdup(real);
            if (d) {
                char *exec_dir = dirname(d);
                char *d2 = strdup(exec_dir);
                if (d2) {
                    char *repo_root = dirname(d2);
                    if (repo_root) chdir(repo_root);
                    free(d2);
                }
                free(d);
            }
        }
    }
}

/* Print brief help */
static void print_help_brief(void) {
    printf("mage_cli usage:\n");
    printf("  help [-all]                 : show this help or all help.json info\n");
    printf("  version                     : show build/runtime version info\n");
    printf("  ping                        : health check (prints 'pong')\n");
    printf("  generate <prompt>           : generate reply for prompt\n");
    printf("  train <file>                : run training pipeline on input file\n");
    printf("  train-pipeline <file>       : run Python-first training pipeline\n");
    printf("  generate-vocab <in> <out>   : build vocab from input and write out\n");
    printf("  validate-vocab <file>       : validate a vocab JSONL file\n");
    printf("  vectorize-py --input <file> : run python vectorizer to produce data/vectors.bin\n");
    printf("  vectors get <i>             : read vector i from data/vectors.bin (mmapped)\n");
    printf("  vectors close               : close mmapped vectors handle\n");
    printf("  retrieve-text <text> [k]    : retrieve nearest neighbors for given text\n");
    printf("  tokenize <text>             : tokenize text via tokenizer API\n");
    printf("  manifest-load <path>        : load manifest file and show entry count\n");
    printf("  manifest-get <path> <idx>   : print manifest entry at index <idx>\n");
    printf("  mphf-probe <path>           : probe MPHF file loadability\n");
    printf("  scrape <url> <out.jsonl>    : run scraper and write JSONL via scraper\n");
    printf("  exit                        : quit interactive mode\n");
}

/* Central command dispatcher (single-line commands) */
static void handle_command_line(const char *line) {
    if (!line) return;
    char buf[LINEBUF_SZ];
    strncpy(buf, line, sizeof(buf)-1);
    buf[sizeof(buf)-1] = '\0';
    trim_inplace(buf);
    if (buf[0] == '\0') return;
    add_history(buf);

    /* Tokenize first word */
    char *saveptr = NULL;
    char *cmd = strtok_r(buf, " ", &saveptr);
    if (!cmd) return;

    /* Built-in commands */
    if (strcmp(cmd, "exit") == 0 || strcmp(cmd, "quit") == 0) {
        mage_api_shutdown();
        free_history();
        exit(0);
    }

    if (strcmp(cmd, "help") == 0) {
        char *arg = strtok_r(NULL, " ", &saveptr);
        if (arg && strcmp(arg, "-all") == 0) {
            if (mage_api_print_help_all() != 0) {
                fprintf(stderr, "No help.json found (looked in data/help.json and ../data/help.json)\n");
            }
        } else {
            print_help_brief();
        }
        return;
    }

    if (strcmp(cmd, "version") == 0) {
        printf("mage_cli (build): OK\n");
        return;
    }

    if (strcmp(cmd, "ping") == 0) {
        printf("pong\n");
        return;
    }

    /* Contract commands */
    if (strcmp(cmd, "contract") == 0) {
        char *sub = strtok_r(NULL, " ", &saveptr);
        if (!sub) { fprintf(stderr, "usage: contract set-cache <n> | contract flush\n"); return; }
        if (strcmp(sub, "set-cache") == 0) {
            char *nstr = strtok_r(NULL, " ", &saveptr);
            if (!nstr) { fprintf(stderr, "usage: contract set-cache <n>\n"); return; }
            size_t entries = (size_t)strtoul(nstr, NULL, 10);
            if (entries == 0) { fprintf(stderr, "invalid cache size\n"); return; }
            if (mage_api_contract_set_cache(entries) != 0) fprintf(stderr, "failed to set cache size\n");
            else printf("cache set to %zu entries\n", entries);
            return;
        } else if (strcmp(sub, "flush") == 0) {
            mage_api_contract_flush();
            printf("contract cache flushed\n");
            return;
        } else {
            fprintf(stderr, "Unknown contract command. Supported: set-cache <n>, flush\n");
            return;
        }
    }

    /* vectors commands */
    if (strcmp(cmd, "vectors") == 0) {
        char *sub = strtok_r(NULL, " ", &saveptr);
        if (!sub) { fprintf(stderr, "vectors commands: get <i>, close\n"); return; }
        static mmapped_index_t *mm_idx = NULL;
        if (strcmp(sub, "get") == 0) {
            char *idxs = strtok_r(NULL, " ", &saveptr);
            if (!idxs) { fprintf(stderr, "usage: vectors get <i>\n"); return; }
            uint32_t idx = (uint32_t)strtoul(idxs, NULL, 10);
            if (!mm_idx) {
                mm_idx = mage_api_mmapped_open("data/vectors.bin");
                if (!mm_idx) { fprintf(stderr, "No vectors index available\n"); return; }
            }
            uint32_t dim = mage_api_mmapped_dim(mm_idx);
            if (dim == 0) dim = 8;
            float *out = malloc(sizeof(float) * dim);
            if (!out) { fprintf(stderr, "allocation failed\n"); return; }
            int r = mage_api_mmapped_get(mm_idx, idx, out);
            if (r != 0) { fprintf(stderr, "Failed to fetch vector %u (rc=%d)\n", idx, r); free(out); return; }
            printf("vector[%u]:", idx);
            for (uint32_t i = 0; i < dim; ++i) printf(" %f", out[i]);
            printf("\n");
            free(out);
            return;
        } else if (strcmp(sub, "close") == 0) {
            printf("vectors close requested (no-op in this session)\n");
            return;
        } else {
            fprintf(stderr, "vectors commands: get <i>, close\n");
            return;
        }
    }

    /* tokenize */
    if (strcmp(cmd, "tokenize") == 0) {
        char *rest = strtok_r(NULL, "", &saveptr);
        if (!rest) { fprintf(stderr, "usage: tokenize <text>\n"); return; }
        trim_inplace(rest);
        char **toks = NULL; size_t count = 0;
        if (mage_api_tokenize(rest, &toks, &count) != 0) { fprintf(stderr, "tokenize failed\n"); return; }
        printf("%zu tokens:\n", count);
        for (size_t i = 0; i < count; ++i) printf("- %s\n", toks[i]);
        mage_api_free_tokens(toks, count);
        return;
    }

    /* generate */
    if (strcmp(cmd, "generate") == 0) {
        char *rest = strtok_r(NULL, "", &saveptr);
        if (!rest) { fprintf(stderr, "usage: generate <prompt>\n"); return; }
        trim_inplace(rest);
        char *out = mage_api_generate(rest);
        if (out) { printf("%s\n", out); free(out); } else { fprintf(stderr, "generation failed\n"); }
        return;
    }

    /* retrieve-text */
    if (strcmp(cmd, "retrieve-text") == 0) {
        char *text = strtok_r(NULL, " ", &saveptr);
        if (!text) { fprintf(stderr, "usage: retrieve-text <text> [k]\n"); return; }
        char *kstr = strtok_r(NULL, " ", &saveptr);
        int k = 5;
        if (kstr) k = atoi(kstr);
        uint32_t dim = 128;
        float *buf = malloc(sizeof(float) * dim);
        if (!buf) { fprintf(stderr, "alloc failed\n"); return; }
        if (mage_api_vectorize_text(text, buf, dim) != 0) { fprintf(stderr, "vectorize failed\n"); free(buf); return; }
        char *js = mage_api_retrieve(buf, dim, (uint32_t)k);
        free(buf);
        if (!js) { printf("{}\n"); return; }
        printf("%s\n", js);
        free(js);
        return;
    }

    /* manifest commands */
    if (strcmp(cmd, "manifest-load") == 0) {
        char *path = strtok_r(NULL, " ", &saveptr);
        if (!path) { fprintf(stderr, "usage: manifest-load <path>\n"); return; }
        manifest_t *m = mage_api_manifest_load(path);
        if (!m) { fprintf(stderr, "failed to load manifest\n"); return; }
        printf("manifest has %u entries\n", m->n);
        mage_api_manifest_free(m);
        return;
    }
    if (strcmp(cmd, "manifest-get") == 0) {
        char *path = strtok_r(NULL, " ", &saveptr);
        char *idxs = strtok_r(NULL, " ", &saveptr);
        if (!path) { fprintf(stderr, "usage: manifest-get <path> <idx>\n"); return; }
        uint32_t idx = 0;
        if (idxs) idx = (uint32_t)atoi(idxs);
        manifest_t *m = mage_api_manifest_load(path);
        if (!m) { fprintf(stderr, "failed to load manifest\n"); return; }
        const manifest_entry_t *e = mage_api_manifest_get(m, idx);
        if (!e) { fprintf(stderr, "no entry at index %u\n", idx); mage_api_manifest_free(m); return; }
        printf("index=%u id=%s title=%s path=%s\n", e->index, e->id ? e->id : "", e->title ? e->title : "", e->path ? e->path : "");
        mage_api_manifest_free(m);
        return;
    }

    /* mphf-probe */
    if (strcmp(cmd, "mphf-probe") == 0) {
        char *path = strtok_r(NULL, " ", &saveptr);
        if (!path) { fprintf(stderr, "usage: mphf-probe <path>\n"); return; }
        int ok = mage_api_mphf_probe(path);
        printf("mphf-probe: %s\n", ok ? "ok" : "failed");
        return;
    }

    /* context-load */
    if (strcmp(cmd, "context-load") == 0) {
        char *path = strtok_r(NULL, " ", &saveptr);
        if (!path) { fprintf(stderr, "usage: context-load <path>\n"); return; }
        int rc = mage_api_context_load_from_jsonl(path);
        if (rc < 0) { fprintf(stderr, "context load failed\n"); return; }
        printf("Loaded %d context entries\n", rc);
        return;
    }

    /* mmapped-get */
    if (strcmp(cmd, "mmapped-get") == 0) {
        char *path = strtok_r(NULL, " ", &saveptr);
        char *idxs = strtok_r(NULL, " ", &saveptr);
        if (!path || !idxs) { fprintf(stderr, "usage: mmapped-get <path> <index>\n"); return; }
        uint32_t idx = (uint32_t)atoi(idxs);
        mmapped_index_t *mi = mage_api_mmapped_open(path);
        if (!mi) { fprintf(stderr, "failed to open mmapped index %s\n", path); return; }
        uint32_t dim = mage_api_mmapped_dim(mi);
        if (dim == 0) dim = 8;
        float *buf = malloc(sizeof(float) * dim);
        if (!buf) { mage_api_mmapped_close(mi); fprintf(stderr, "alloc failed\n"); return; }
        int r = mage_api_mmapped_get(mi, idx, buf);
        if (r != 0) { fprintf(stderr, "failed to read index %u (rc=%d)\n", idx, r); free(buf); mage_api_mmapped_close(mi); return; }
        printf("vector[%u]:", idx);
        for (uint32_t j = 0; j < dim; ++j) printf(" %f", buf[j]);
        printf("\n");
        free(buf); mage_api_mmapped_close(mi);
        return;
    }

    /* build-vocab */
    if (strcmp(cmd, "build-vocab") == 0) {
        char *in = strtok_r(NULL, " ", &saveptr);
        char *out = strtok_r(NULL, " ", &saveptr);
        char *minc = strtok_r(NULL, " ", &saveptr);
        if (!in || !out) { fprintf(stderr, "usage: build-vocab <input> <out.jsonl> [min_count]\n"); return; }
        int min_count = 1;
        if (minc) min_count = atoi(minc);
        int rc = mage_api_build_vocab_from_file(in, out, min_count);
        if (rc < 0) { fprintf(stderr, "build vocab failed\n"); return; }
        printf("Wrote %d vocab entries to %s\n", rc, out);
        return;
    }

    /* generate-vocab */
    if (strcmp(cmd, "generate-vocab") == 0) {
        char *in = strtok_r(NULL, " ", &saveptr);
        char *out = strtok_r(NULL, " ", &saveptr);
        if (!in || !out) { fprintf(stderr, "usage: generate-vocab <train_input> <out.jsonl>\n"); return; }
        int added = mage_api_train_from_file(in);
        if (added < 0) { fprintf(stderr, "Training/generation failed for '%s'\n", in); return; }
        FILE *src = fopen("data/vocab/generated_vocab.jsonl", "r");
        if (!src) { fprintf(stderr, "No generated vocab at data/vocab/generated_vocab.jsonl\n"); return; }
        FILE *dst = fopen(out, "w");
        if (!dst) { fclose(src); fprintf(stderr, "failed to open %s for write\n", out); return; }
        char buf2[8192]; size_t n;
        while ((n = fread(buf2, 1, sizeof(buf2), src)) > 0) fwrite(buf2, 1, n, dst);
        fclose(src); fclose(dst);
        printf("Wrote generated vocab to %s\n", out);
        return;
    }

    /* validate-vocab (fallback to python tool) */
    if (strcmp(cmd, "validate-vocab") == 0 || strcmp(cmd, "validate-generated-vocab") == 0) {
        char *path = strtok_r(NULL, " ", &saveptr);
        if (!path) path = "data/vocab/generated_vocab.jsonl";
        char cmdline[4096];
        snprintf(cmdline, sizeof(cmdline), "python3 tools/validate_vocab.py %s", path);
        int rc = system(cmdline);
        if (rc == 0) printf("vocab validated\n"); else fprintf(stderr, "vocab validation failed\n");
        return;
    }

    /* train-pipeline (python) */
    if (strcmp(cmd, "train-pipeline") == 0) {
        char *in = strtok_r(NULL, " ", &saveptr);
        if (!in) { fprintf(stderr, "usage: train-pipeline <input.jsonl>\n"); return; }
        char cmdline[8192];
        snprintf(cmdline, sizeof(cmdline), "python3 python/tools/train_pipeline.py --input '%s' --vocab-out data/vocab/generated_vocab.jsonl", in);
        (void)system(cmdline);
        return;
    }

    /* train (prefer python pipeline if available) */
    if (strcmp(cmd, "train") == 0) {
        char *in = strtok_r(NULL, " ", &saveptr);
        if (!in) { fprintf(stderr, "usage: train <file> [--py]\n"); return; }
        char *opt = strtok_r(NULL, " ", &saveptr);
        int use_py = 0;
        if (opt && strcmp(opt, "--py") == 0) use_py = 1;
        if (use_py || access("python/tools/train_pipeline.py", F_OK) == 0) {
            char cmdline[8192];
            snprintf(cmdline, sizeof(cmdline), "python3 python/tools/train_pipeline.py --input '%s' --vocab-out data/vocab/generated_vocab.jsonl", in);
            (void)system(cmdline);
            return;
        }
        int added = mage_api_train_from_file(in);
        if (added < 0) { fprintf(stderr, "Training failed for '%s'\n", in); return; }
        printf("Training complete: %d entries added from '%s'\n", added, in);
        return;
    }

    /* mphf-gen (external tools) */
    if (strcmp(cmd, "mphf-gen") == 0) {
        char *in = strtok_r(NULL, " ", &saveptr);
        char *out = strtok_r(NULL, " ", &saveptr);
        if (!in) { fprintf(stderr, "usage: mphf-gen <vocab.jsonl> [out.bin]\n"); return; }
        if (!out) out = "data/mphf.bin";
        if (access("tools/gen_mphf_chd", X_OK) == 0) {
            char cmdline[4096]; snprintf(cmdline, sizeof(cmdline), "tools/gen_mphf_chd %s %s", in, out); system(cmdline); return;
        }
        if (access("tools/gen_mphf", X_OK) == 0) {
            char cmdline[4096]; snprintf(cmdline, sizeof(cmdline), "tools/gen_mphf %s %s", in, out); system(cmdline); return;
        }
        fprintf(stderr, "gen_mphf tool not found; build tools/gen_mphf or tools/gen_mphf_chd\n");
        return;
    }

    /* scrape (python tool) */
    if (strcmp(cmd, "scrape") == 0) {
        char *url = strtok_r(NULL, " ", &saveptr);
        char *out = strtok_r(NULL, " ", &saveptr);
        if (!url || !out) { fprintf(stderr, "usage: scrape <url> <out.jsonl>\n"); return; }
        char cmdline[4096];
        snprintf(cmdline, sizeof(cmdline), "python3 python/tools/scraper.py --url '%s' --out '%s'", url, out);
        int rc = system(cmdline);
        if (rc == 0) printf("Scrape complete\n"); else fprintf(stderr, "Scrape failed (rc=%d)\n", rc);
        return;
    }

    /* fallback: try to parse prompt and finalize reply via API */
    {
        char *resp = mage_api_parse_prompt(line);
        if (resp) {
            char *final = mage_api_finalize_reply(resp);
            if (final) {
                printf("%s\n", final);
                if (strcmp(final, "Goodbye.") == 0) {
                    free(final); free(resp); mage_api_shutdown(); free_history(); exit(0);
                }
                free(final);
            } else {
                printf("%s\n", resp);
            }
            free(resp);
        } else {
            fprintf(stderr, "(error parsing prompt)\n");
        }
    }
}

/* Interactive REPL */
static void interactive_mode(void) {
    char *line = NULL;
    size_t cap = 0;
    ssize_t nread;

    signal(SIGINT, handle_sigint);

    printf("Mage CLI (type 'exit' or Ctrl-D to quit)\n");
    while (1) {
        printf("> ");
        fflush(stdout);
        nread = getline(&line, &cap, stdin);
        if (nread == -1) break;
        if (nread > 0 && (line[nread-1] == '\n' || line[nread-1] == '\r')) line[--nread] = '\0';
        trim_inplace(line);
        if (line[0] == '\0') continue;
        handle_command_line(line);
    }
    free(line);
}

/* Read existing runtime overrides (if present). Returns 1 if file present, 0 otherwise. */
static int read_runtime_overrides(unsigned long long *out_k, unsigned long long *out_seed) {
    const char* path = "config/mage_runtime_overrides.json";
    FILE* f = fopen(path, "r");
    if (!f) return 0;
    fseek(f, 0, SEEK_END); long sz = ftell(f); fseek(f, 0, SEEK_SET);
    if (sz <= 0) { fclose(f); return 0; }
    char *buf = malloc((size_t)sz + 1);
    if (!buf) { fclose(f); return 0; }
    size_t n = fread(buf, 1, (size_t)sz, f); buf[n] = '\0'; fclose(f);
    jsmn_parser p; jsmn_init(&p); jsmntok_t toks[256];
    int nt = jsmn_parse(&p, buf, n, toks, 256);
    int found = 0;
    if (nt > 0) {
        for (int i = 0; i < nt; ++i) {
            jsmntok_t *t = &toks[i];
            if (t->type == JSMN_STRING) {
                int len = t->end - t->start;
                if (len == 4 && strncmp(buf + t->start, "rp_K", 4) == 0 && i+1 < nt) {
                    jsmntok_t *v = &toks[i+1];
                    char tmp[64];
                    int l = v->end - v->start;
                    if (l >= (int)sizeof(tmp)) l = sizeof(tmp)-1;
                    memcpy(tmp, buf + v->start, l); tmp[l] = '\0';
                    *out_k = strtoull(tmp, NULL, 0);
                    found = 1;
                } else if (len == 9 && strncmp(buf + t->start, "hash_seed", 9) == 0 && i+1 < nt) {
                    jsmntok_t *v = &toks[i+1];
                    char tmp[64];
                    int l = v->end - v->start;
                    if (l >= (int)sizeof(tmp)) l = sizeof(tmp)-1;
                    memcpy(tmp, buf + v->start, l); tmp[l] = '\0';
                    *out_seed = strtoull(tmp, NULL, 0);
                    found = 1;
                }
            }
        }
    }
    free(buf);
    return found;
}

/* Main entry */
int main(int argc, char **argv) {
    ensure_repo_root(argc, argv);

    /* parse lightweight flags */
    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--verbose") == 0) {
            verbose_mode = 1;
            mage_log_init(MAGE_LOG_DEBUG, "data/full_verbose.log");
            mage_log_set_level(MAGE_LOG_DEBUG);
        }
        if (strncmp(argv[i], "--rp-k=", 7) == 0) {
            const char *val = argv[i] + 7;
            setenv("MAGE_RP_K", val, 1);
            /* persist to config file */
            unsigned long long tmp_k = 0, tmp_seed = 0;
            (void)read_runtime_overrides(&tmp_k, &tmp_seed);
            FILE *out = fopen("config/mage_runtime_overrides.json", "w");
            if (out) {
                unsigned long long k = (unsigned long long)strtoull(val, NULL, 10);
                fprintf(out, "{\n  \"tap\": { \"rp_K\": %llu },\n  \"random_projection\": { \"hash_seed\": %llu }\n}\n", k, tmp_seed);
                fclose(out);
            }
        }
        if (strncmp(argv[i], "--rp-seed=", 10) == 0) {
            const char *val = argv[i] + 10;
            setenv("MAGE_RP_SEED", val, 1);
            unsigned long long tmp_k = 0, tmp_seed = 0;
            (void)read_runtime_overrides(&tmp_k, &tmp_seed);
            FILE *out = fopen("config/mage_runtime_overrides.json", "w");
            if (out) {
                unsigned long long s = strtoull(val, NULL, 0);
                fprintf(out, "{\n  \"tap\": { \"rp_K\": %llu },\n  \"random_projection\": { \"hash_seed\": %llu }\n}\n", tmp_k, s);
                fclose(out);
            }
        }
    }

    /* Initialize API subsystems */
    (void)mage_api_init();

    /* Try to load context history if present */
    const char *ctx_path = "data/context_history.jsonl";
    FILE *cf = fopen(ctx_path, "r");
    if (cf) {
        fclose(cf);
        int got = mage_api_context_load_from_jsonl(ctx_path);
        if (got > 0) fprintf(stderr, "Loaded %d context entries from %s\n", got, ctx_path);
    }

    /* Probe MPHF if present */
    const char *mphf_path = "data/mphf.bin";
    FILE *mf = fopen(mphf_path, "rb");
    if (mf) {
        fclose(mf);
        int ok = mage_api_mphf_probe(mphf_path);
        if (ok) fprintf(stderr, "Loaded MPHF from %s\n", mphf_path);
        else fprintf(stderr, "Failed to load MPHF from %s\n", mphf_path);
    }

    /* If arguments provided, treat them as a single command (non-interactive) */
    if (argc > 1) {
        /* Reconstruct command from argv[1..] */
        size_t total = 0;
        for (int i = 1; i < argc; ++i) total += strlen(argv[i]) + 1;
        char *cmdline = malloc(total + 1);
        if (!cmdline) return 1;
        cmdline[0] = '\0';
        for (int i = 1; i < argc; ++i) {
            strcat(cmdline, argv[i]);
            if (i < argc - 1) strcat(cmdline, " ");
        }
        handle_command_line(cmdline);
        free(cmdline);
        mage_api_shutdown();
        free_history();
        return 0;
    }

    /* No args: interactive mode */
    interactive_mode();

    /* Clean shutdown */
    mage_api_shutdown();
    free_history();
    return 0;
}
