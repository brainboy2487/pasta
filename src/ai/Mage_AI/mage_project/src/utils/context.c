/* context.c - lightweight JSONL conversation history loader
 * Accepts lines of simple JSON objects and extracts 'role' and 'text' fields.
 * This is intentionally permissive: if parsing fails for a line we store the
 * entire line as a 'user' text entry.
 */

#define _POSIX_C_SOURCE 200809L

#include "mage/context.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    char* role;
    char* text;
} ctx_entry_t;

static ctx_entry_t* CTX = NULL;
static size_t CTX_N = 0;

static char* strdup_safe(const char* s) {
    if (!s) return NULL;
    size_t l = strlen(s);
    char* t = malloc(l + 1);
    if (!t) return NULL;
    memcpy(t, s, l + 1);
    return t;
}

void mage_context_clear(void) {
    for (size_t i = 0; i < CTX_N; ++i) {
        free(CTX[i].role);
        free(CTX[i].text);
    }
    free(CTX);
    CTX = NULL; CTX_N = 0;
}

size_t mage_context_count(void) { return CTX_N; }

const char* mage_context_get_role(size_t idx) {
    if (idx >= CTX_N) return NULL;
    return CTX[idx].role;
}

const char* mage_context_get_text(size_t idx) {
    if (idx >= CTX_N) return NULL;
    return CTX[idx].text;
}

/* naive JSON substring extractor for a field name: returns pointer to start
 * of value (inside quotes) or NULL. Caller should not free. */
static const char* find_json_string_value(const char* line, const char* field) {
    const char* p = strstr(line, field);
    if (!p) return NULL;
    p = strchr(p, ':'); if (!p) return NULL; ++p;
    while (*p && (*p == ' ' || *p == '\t')) ++p;
    if (*p == '"') {
        ++p; return p;
    }
    return NULL;
}

int mage_context_load_from_jsonl(const char* path) {
    if (!path) return -1;
    FILE* f = fopen(path, "r");
    if (!f) return -1;

    char buf[8192];
    size_t added = 0;
    while (fgets(buf, sizeof(buf), f)) {
        size_t len = strlen(buf);
        while (len > 0 && (buf[len-1] == '\n' || buf[len-1] == '\r')) { buf[--len] = '\0'; }

        const char* role_start = find_json_string_value(buf, "\"role\"");
        const char* text_start = find_json_string_value(buf, "\"text\"");

        char role_tmp[32]; role_tmp[0] = '\0';
        char* text_tmp = NULL;

        if (text_start) {
            const char* t = text_start;
            const char* tend = strchr(t, '"');
            if (tend) {
                size_t tlen = (size_t)(tend - t);
                text_tmp = malloc(tlen + 1);
                if (text_tmp) { memcpy(text_tmp, t, tlen); text_tmp[tlen] = '\0'; }
            }
        }
        if (role_start) {
            const char* r = role_start;
            const char* rend = strchr(r, '"');
            if (rend) {
                size_t rlen = (size_t)(rend - r);
                if (rlen >= sizeof(role_tmp)) rlen = sizeof(role_tmp) - 1;
                memcpy(role_tmp, r, rlen); role_tmp[rlen] = '\0';
            }
        }

        if (!text_tmp) {
            /* fallback: store the whole line as text */
            text_tmp = strdup_safe(buf);
            if (!text_tmp) continue;
        }

        const char* role_final = role_tmp[0] ? role_tmp : "user";

        ctx_entry_t* n = realloc(CTX, (CTX_N + 1) * sizeof(ctx_entry_t));
        if (!n) { free(text_tmp); continue; }
        CTX = n;
        CTX[CTX_N].role = strdup_safe(role_final);
        CTX[CTX_N].text = text_tmp;
        CTX_N++; added++;
    }
    fclose(f);
    return (int)added;
}
