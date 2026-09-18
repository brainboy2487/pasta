/* tokenizer.c - improved tokenization utilities for Mage
 * Generated: 2026-01-23
 */

#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <stdint.h>
#include <stdbool.h>
#include "mage/tokenizer.h"

#include <stdio.h>
#ifdef HAVE_UTF8PROC
#include <utf8proc.h>
#endif

/* Basic UTF-8 helpers: decode codepoint from bytes, returning byte length. */
static int utf8_decode(const char *s, uint32_t *out_cp) {
    unsigned char c = (unsigned char)s[0];
    if (c < 0x80) { *out_cp = c; return 1; }
    if ((c & 0xE0) == 0xC0) {
        if (((unsigned char)s[1] & 0xC0) == 0x80) {
            *out_cp = ((c & 0x1F) << 6) | ((unsigned char)s[1] & 0x3F); return 2;
        }
        return 1;
    }
    if ((c & 0xF0) == 0xE0) {
        *out_cp = ((c & 0x0F) << 12) | (((unsigned char)s[1] & 0x3F) << 6) | ((unsigned char)s[2] & 0x3F); return 3;
    }
    if ((c & 0xF8) == 0xF0) {
        *out_cp = ((c & 0x07) << 18) | (((unsigned char)s[1] & 0x3F) << 12) | (((unsigned char)s[2] & 0x3F) << 6) | ((unsigned char)s[3] & 0x3F); return 4;
    }
    /* invalid or unsupported sequence */
    *out_cp = c; return 1;
}

/* Append a single ASCII char to output buffer (grow allocated buffer as needed) */
static void out_append_char(char **out, size_t *olen, size_t *cap, char c) {
    if (*olen + 1 >= *cap) {
        size_t nc = (*cap == 0) ? 256 : (*cap * 2);
        char *t = realloc(*out, nc);
        if (!t) return;
        *out = t; *cap = nc;
    }
    (*out)[(*olen)++] = c;
}

/* Append raw UTF-8 bytes to output */
static void out_append_bytes(char **out, size_t *olen, size_t *cap, const char *s, size_t n) {
    if (*olen + n >= *cap) {
        size_t nc = (*cap == 0) ? 256 : (*cap * 2);
        while (*olen + n >= nc) nc *= 2;
        char *t = realloc(*out, nc);
        if (!t) return;
        *out = t; *cap = nc;
    }
    memcpy(*out + *olen, s, n);
    *olen += n;
}

/* Normalize: keep alnum and spaces, lower-case */
char* mage_normalize(const char* s) {
    if (!s) return NULL;

#ifdef HAVE_UTF8PROC
    /* Use utf8proc to perform NFKC + case-folding when available. */
    utf8proc_uint8_t *mapped = NULL;
    mapped = utf8proc_NFKC_Casefold((const utf8proc_uint8_t*)s);
    if (mapped) {
        const char *p = (const char*)mapped;
        char *out = NULL; size_t olen = 0, cap = 0;
        bool last_was_space = false;

        while (*p) {
            uint32_t cp = 0;
            int bl = utf8_decode(p, &cp);
            if (cp <= 0x7F && isspace((unsigned char)cp)) {
                if (!last_was_space) out_append_char(&out, &olen, &cap, ' ');
                last_was_space = true;
            } else if (cp <= 0x7F) {
                /* ASCII non-space: copy lowercased ASCII where appropriate */
                unsigned char c = (unsigned char)cp;
                if (isalnum(c)) { out_append_char(&out, &olen, &cap, (char)tolower(c)); last_was_space = false; }
                else if (c == '\'') { last_was_space = false; }
                else if (c == '-') { out_append_char(&out, &olen, &cap, '-'); last_was_space = false; }
                else { /* drop punctuation */ last_was_space = last_was_space; }
            } else {
                /* Non-ASCII: apply same pragmatic mappings as fallback */
                if (cp == 0x00A0) { if (!last_was_space) out_append_char(&out, &olen, &cap, ' '); last_was_space = true; }
                else if (cp == 0x2018 || cp == 0x2019 || cp == 0x201A) { /* curly single quotes */ last_was_space = false; }
                else if (cp == 0x201C || cp == 0x201D) { /* curly double quotes */ last_was_space = false; }
                else if (cp == 0x2013 || cp == 0x2014) { out_append_char(&out, &olen, &cap, '-'); last_was_space = false; }
                else if (cp >= 0xFF01 && cp <= 0xFF5E) {
                    uint32_t asc = cp - 0xFF00 + 0x20;
                    if (asc <= 0x7F && isprint((unsigned char)asc)) out_append_char(&out, &olen, &cap, (char)tolower((unsigned char)asc));
                    last_was_space = false;
                } else {
                    out_append_bytes(&out, &olen, &cap, p, (size_t)bl);
                    last_was_space = false;
                }
            }
            p += bl;
        }
        if (olen > 0 && out[olen-1] == ' ') olen--;
        out_append_char(&out, &olen, &cap, '\0');
        free(mapped);
        return out;
    }
#endif

    /* Fallback pragmatic normalizer */
    const char *p = s;
    char *out = NULL; size_t olen = 0, cap = 0;
    bool last_was_space = false;

    while (*p) {
        uint32_t cp = 0;
        int bl = utf8_decode(p, &cp);
        /* Map common codepoints to ASCII equivalents when possible */
        if (cp <= 0x7F) {
            char c = (char)cp;
            if (isspace((unsigned char)c)) {
                if (!last_was_space) out_append_char(&out, &olen, &cap, ' ');
                last_was_space = true;
            } else if (isalnum((unsigned char)c)) {
                out_append_char(&out, &olen, &cap, (char)tolower((unsigned char)c));
                last_was_space = false;
            } else if (c == '\'') {
                /* drop lone apostrophes */ last_was_space = false; }
            else if (c == '-') { out_append_char(&out, &olen, &cap, '-'); last_was_space = false; }
            else {
                /* drop other ASCII punctuation */ last_was_space = last_was_space;
            }
        } else {
            /* Non-ASCII: map some common punctuation/codepoints (NBSP, curly quotes, em/en dash, fullwidth ASCII) */
            if (cp == 0x00A0) { if (!last_was_space) out_append_char(&out, &olen, &cap, ' '); last_was_space = true; }
            else if (cp == 0x2018 || cp == 0x2019 || cp == 0x201A) { /* curly single quotes */ last_was_space = false; }
            else if (cp == 0x201C || cp == 0x201D) { /* curly double quotes */ last_was_space = false; }
            else if (cp == 0x2013 || cp == 0x2014) { out_append_char(&out, &olen, &cap, '-'); last_was_space = false; }
            else if (cp >= 0xFF01 && cp <= 0xFF5E) {
                /* fullwidth ASCII range: map to ASCII (cp - 0xFF00 + 0x20) */
                uint32_t asc = cp - 0xFF00 + 0x20;
                if (asc <= 0x7F && isprint((unsigned char)asc)) out_append_char(&out, &olen, &cap, (char)tolower((unsigned char)asc));
                last_was_space = false;
            } else {
                /* preserve UTF-8 sequences for tokens: copy raw bytes */
                out_append_bytes(&out, &olen, &cap, p, (size_t)bl);
                last_was_space = false;
            }
        }
        p += bl;
    }
    /* trim trailing space */
    if (olen > 0 && out[olen-1] == ' ') olen--;
    out_append_char(&out, &olen, &cap, '\0');
    return out;
}

/* Tokenization: allow letters, digits, apostrophes and hyphens inside tokens. */
int mage_tokenize(const char* text, char*** tokens_out, size_t* count_out) {
    if (!text || !tokens_out || !count_out) return -1;
    *tokens_out = NULL; *count_out = 0;

    /* First normalize (collapses whitespace and maps common symbols) */
    char* norm = mage_normalize(text);
    if (!norm) return -1;

    size_t cap = 0;
    char *token = NULL; size_t tlen = 0, tcap = 0;
    const char *p = norm;

    while (*p) {
        uint32_t cp = 0; int bl = utf8_decode(p, &cp);
        bool is_token_char = false;
        if (cp <= 0x7F) {
            unsigned char c = (unsigned char)cp;
            if (isalnum(c) || c == '\'' || c == '-') is_token_char = true;
        } else {
            /* treat any non-ASCII UTF-8 sequence as token character */
            is_token_char = true;
        }

        if (is_token_char) {
            /* append raw bytes for this character */
            if (tlen + bl + 1 >= tcap) {
                size_t nc = tcap == 0 ? 256 : tcap * 2;
                while (tlen + bl + 1 >= nc) nc *= 2;
                char *nt = realloc(token, nc);
                if (!nt) { free(norm); if (token) free(token); mage_free_tokens(*tokens_out, *count_out); *tokens_out = NULL; *count_out = 0; return -1; }
                token = nt; tcap = nc;
            }
            memcpy(token + tlen, p, bl); tlen += bl; token[tlen] = '\0';
        } else {
            if (tlen > 0) {
                /* commit token */
                if (*count_out + 1 > cap) {
                    size_t newcap = cap == 0 ? 8 : cap * 2;
                    char** tmp = realloc(*tokens_out, newcap * sizeof(char*));
                    if (!tmp) { free(norm); free(token); mage_free_tokens(*tokens_out, *count_out); *tokens_out = NULL; *count_out = 0; return -1; }
                    *tokens_out = tmp; cap = newcap;
                }
                (*tokens_out)[*count_out] = malloc(tlen + 1);
                if (!(*tokens_out)[*count_out]) { free(norm); free(token); mage_free_tokens(*tokens_out, *count_out); *tokens_out = NULL; *count_out = 0; return -1; }
                memcpy((*tokens_out)[*count_out], token, tlen + 1);
                (*count_out)++;
                tlen = 0; token[0] = '\0';
            }
        }
        p += bl;
    }
    if (tlen > 0) {
        if (*count_out + 1 > cap) {
            size_t newcap = cap == 0 ? 8 : cap * 2;
            char** tmp = realloc(*tokens_out, newcap * sizeof(char*));
            if (!tmp) { free(norm); free(token); mage_free_tokens(*tokens_out, *count_out); *tokens_out = NULL; *count_out = 0; return -1; }
            *tokens_out = tmp; cap = newcap;
        }
        (*tokens_out)[*count_out] = malloc(tlen + 1);
        if (!(*tokens_out)[*count_out]) { free(norm); free(token); mage_free_tokens(*tokens_out, *count_out); *tokens_out = NULL; *count_out = 0; return -1; }
        memcpy((*tokens_out)[*count_out], token, tlen + 1);
        (*count_out)++;
    }

    free(token); free(norm);
    return 0;
}

void mage_free_tokens(char** tokens, size_t count) {
    if (!tokens) return;
    for (size_t i = 0; i < count; ++i) free(tokens[i]);
    free(tokens);
}
