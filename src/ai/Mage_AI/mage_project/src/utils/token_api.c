/* token_api.c - Tokenizer API wrappers for MAGE
 * Provides C-level vocab building and tokenization utilities wired into mage_api.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include "mage/tokenizer.h"

static int cmp_ent(const void* pa, const void* pb) {
    const struct ent_t_placeholder { char* token; size_t count; } *a = pa, *b = pb;
    if (a->count < b->count) return 1;
    if (a->count > b->count) return -1;
    return strcmp(a->token, b->token);
}

/* Expose tokenizer via API-level functions used by presentation */
int mage_api_tokenize(const char* text, char*** tokens_out, size_t* count_out) {
    return mage_tokenize(text, tokens_out, count_out);
}

void mage_api_free_tokens(char** tokens, size_t count) {
    mage_free_tokens(tokens, count);
}

/* Minimal JSONL text extractor: looks for common fields 'text','content','body' */
static int extract_text_from_jsonl_line(const char* line, char* outbuf, size_t cap) {
    const char* keys[] = {"\"text\"", "\"content\"", "\"body\""};
    for (size_t k=0;k<sizeof(keys)/sizeof(keys[0]);++k) {
        const char* p = strstr(line, keys[k]);
        if (!p) continue;
        const char* q = strchr(p, ':'); if (!q) continue; q++;
        while (*q && isspace((unsigned char)*q)) q++;
        if (*q == '"') q++; else continue;
        const char* e = strchr(q, '"'); if (!e) continue;
        size_t n = (size_t)(e - q);
        if (n >= cap) n = cap-1;
        memcpy(outbuf, q, n); outbuf[n] = '\0'; return 1;
    }
    return 0;
}

int mage_api_build_vocab_from_file(const char* input_path, const char* out_path, int min_count) {
    if (!input_path || !out_path) return -1;
    FILE* f = fopen(input_path, "r");
    if (!f) return -1;
    /* simple hash map via chaining using sorted array is heavy; use dynamic arrays and qsort at end */
    typedef struct { char* token; size_t count; } ent_t;
    ent_t *ents = NULL; size_t ecap=0, elen=0;
    char line[4096]; char textbuf[4096];
    while (fgets(line, sizeof(line), f)) {
        if (strchr(line,'\n')) {}
        int got = 0;
        if (strchr(input_path, '.') && strstr(input_path, ".jsonl")) {
            got = extract_text_from_jsonl_line(line, textbuf, sizeof(textbuf));
        }
        if (!got) {
            /* treat as plain text */
            strncpy(textbuf, line, sizeof(textbuf)-1); textbuf[sizeof(textbuf)-1]='\0';
        }
        /* tokenize */
        char **tokens = NULL; size_t tcount = 0;
        if (mage_tokenize(textbuf, &tokens, &tcount) != 0) continue;
        for (size_t i=0;i<tcount;++i) {
            char *t = tokens[i];
            /* find or insert */
            size_t j; for (j=0;j<elen;++j) { if (strcmp(ents[j].token, t)==0) { ents[j].count++; break; } }
            if (j==elen) {
                if (elen+1 > ecap) { size_t nc = ecap? ecap*2 : 256; ent_t *n = realloc(ents, nc * sizeof(ent_t)); if (!n) { mage_free_tokens(tokens, tcount); fclose(f); return -1; } ents = n; ecap = nc; }
                size_t tn = strlen(t) + 1;
                ents[elen].token = malloc(tn);
                if (ents[elen].token) memcpy(ents[elen].token, t, tn);
                ents[elen].count = 1; elen++;
            }
        }
        mage_free_tokens(tokens, tcount);
    }
    fclose(f);
    /* filter and sort by count desc */
    if (elen==0) return -1;
    qsort(ents, elen, sizeof(ent_t), cmp_ent);
    /* write out */
    FILE *out = fopen(out_path, "w"); if (!out) { for (size_t i=0;i<elen;++i) free(ents[i].token); free(ents); return -1; }
    size_t written = 0;
    for (size_t i=0;i<elen;++i) {
        if ((int)ents[i].count < min_count) continue;
        /* Emit vocab entries in the expected protocol for parser: include id, key, and responses */
        /* Use token as id and key; responses default to the token itself to make a minimal entry */
        fprintf(out, "{\"id\": \"%s\", \"key\": \"%s\", \"responses\": [\"%s\"], \"type\": \"other\"}\n", ents[i].token, ents[i].token, ents[i].token);
        written++;
    }
    fclose(out);
    for (size_t i=0;i<elen;++i) free(ents[i].token);
    free(ents);
    return (int)written;
}
