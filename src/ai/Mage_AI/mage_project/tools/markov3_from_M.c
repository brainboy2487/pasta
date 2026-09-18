// markov3_from_M.c
// Robust parser for lines like:
//   token:maximum -> of:2 -> kutuzov:1
// Produces a 3rd-order Markov table exported to MHL1.txt
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

#define HASH_SIZE 16384
#define MAX_LINE 8192
#define MAX_PARTS 1024

static char *xstrdup(const char *s) {
    size_t n = strlen(s) + 1;
    char *p = malloc(n);
    if (!p) { fprintf(stderr, "OOM\n"); exit(1); }
    memcpy(p, s, n);
    return p;
}

typedef struct NextNode {
    char *token;
    int count;
    struct NextNode *next;
} NextNode;

typedef struct TriNode {
    char *a;
    char *b;
    NextNode *nexts;
    struct TriNode *next;
} TriNode;

static TriNode *table[HASH_SIZE];

static unsigned int hash_pair(const char *a, const char *b) {
    unsigned int h = 2166136261u;
    const unsigned char *p;
    for (p = (const unsigned char*)a; *p; ++p) h = (h ^ *p) * 16777619u;
    for (p = (const unsigned char*)b; *p; ++p) h = (h ^ *p) * 16777619u;
    return h & (HASH_SIZE - 1);
}

static TriNode *get_trinode(const char *a, const char *b) {
    unsigned int h = hash_pair(a,b);
    TriNode *n = table[h];
    while (n) {
        if (strcmp(n->a, a) == 0 && strcmp(n->b, b) == 0) return n;
        n = n->next;
    }
    n = malloc(sizeof(TriNode));
    if (!n) { fprintf(stderr, "OOM\n"); exit(1); }
    n->a = xstrdup(a);
    n->b = xstrdup(b);
    n->nexts = NULL;
    n->next = table[h];
    table[h] = n;
    return n;
}

static void add_trigram(const char *a, const char *b, const char *c) {
    TriNode *t = get_trinode(a,b);
    NextNode *n = t->nexts;
    while (n) {
        if (strcmp(n->token, c) == 0) { n->count++; return; }
        n = n->next;
    }
    n = malloc(sizeof(NextNode));
    if (!n) { fprintf(stderr, "OOM\n"); exit(1); }
    n->token = xstrdup(c);
    n->count = 1;
    n->next = t->nexts;
    t->nexts = n;
}

/* trim leading/trailing whitespace and remove trailing '>' or other stray chars */
static void trim_and_clean(char *s) {
    // trim leading
    char *p = s;
    while (*p && isspace((unsigned char)*p)) p++;
    if (p != s) memmove(s, p, strlen(p)+1);

    // trim trailing whitespace
    int len = (int)strlen(s);
    while (len > 0 && isspace((unsigned char)s[len-1])) s[--len] = '\0';

    // remove trailing '>' or stray non-token chars
    while (len > 0) {
        char c = s[len-1];
        if (c == '>' || c == '\x1A' || c == '\r' || c == '\n') { s[--len] = '\0'; continue; }
        // keep apostrophes, hyphens, alnum; if last char is punctuation like ',' '.' '>' '>' remove it
        if (!isalnum((unsigned char)c) && c != '\'' && c != '-' && c != '_') { s[--len] = '\0'; continue; }
        break;
    }
}

/* extract token from a part:
   - if part starts with "token:" remove that prefix and return remainder
   - else if part contains ':' return substring before ':' (token:count)
   - else return whole part
*/
static char *extract_token_from_part(char *part) {
    trim_and_clean(part);
    if (part[0] == '\0') return NULL;

    const char *prefix = "token:";
    if (strncmp(part, prefix, strlen(prefix)) == 0) {
        char *t = part + strlen(prefix);
        trim_and_clean(t);
        if (t[0] == '\0') return NULL;
        return xstrdup(t);
    }

    // find first colon
    char *cpos = strchr(part, ':');
    if (cpos) {
        // token is before colon
        int len = (int)(cpos - part);
        while (len > 0 && isspace((unsigned char)part[len-1])) len--;
        if (len <= 0) return NULL;
        char tmp[512];
        if (len >= (int)sizeof(tmp)) len = (int)sizeof(tmp)-1;
        memcpy(tmp, part, len);
        tmp[len] = '\0';
        trim_and_clean(tmp);
        if (tmp[0] == '\0') return NULL;
        return xstrdup(tmp);
    }

    // no colon, return whole cleaned part
    return xstrdup(part);
}

/* split line by "->" into parts, extract tokens in order */
static int parse_line_to_tokens(char *line, char *out[], int max_out) {
    int count = 0;
    char *p = line;
    // split by "->"
    char *parts[MAX_PARTS];
    int np = 0;

    // manual split to preserve segments even if no spaces
    char *start = p;
    while (*p) {
        char *arrow = strstr(p, "->");
        if (!arrow) {
            parts[np++] = xstrdup(start);
            break;
        } else {
            int seglen = (int)(arrow - start);
            char *seg = malloc(seglen + 1);
            if (!seg) { fprintf(stderr, "OOM\n"); exit(1); }
            memcpy(seg, start, seglen);
            seg[seglen] = '\0';
            parts[np++] = seg;
            p = arrow + 2;
            start = p;
        }
        if (np >= MAX_PARTS-1) break;
    }

    for (int i = 0; i < np && count < max_out; ++i) {
        char *tok = extract_token_from_part(parts[i]);
        free(parts[i]);
        if (tok) {
            out[count++] = tok;
        }
    }
    return count;
}

static void process_file(const char *path) {
    FILE *f = fopen(path, "r");
    if (!f) { fprintf(stderr, "Warning: cannot open %s\n", path); return; }
    char line[MAX_LINE];
    while (fgets(line, sizeof(line), f)) {
        // skip empty lines quickly
        char *s = line;
        while (*s && isspace((unsigned char)*s)) s++;
        if (*s == '\0') continue;

        char *tokens[256];
        int nt = parse_line_to_tokens(line, tokens, 256);
        if (nt >= 3) {
            for (int i = 0; i + 2 < nt; ++i) {
                add_trigram(tokens[i], tokens[i+1], tokens[i+2]);
            }
        }
        for (int i = 0; i < nt; ++i) free(tokens[i]);
    }
    fclose(f);
}

static void export_trigrams(const char *outpath) {
    FILE *out = fopen(outpath, "w");
    if (!out) { fprintf(stderr, "Cannot write %s\n", outpath); return; }
    for (int i = 0; i < HASH_SIZE; ++i) {
        TriNode *n = table[i];
        while (n) {
            fprintf(out, "token:%s,%s", n->a, n->b);
            NextNode *m = n->nexts;
            while (m) {
                fprintf(out, " -> %s:%d", m->token, m->count);
                m = m->next;
            }
            fprintf(out, "\n");
            n = n->next;
        }
    }
    fclose(out);
}

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s M1_data.txt [M2_data.txt ...]\n", argv[0]);
        return 1;
    }
    for (int i = 1; i < argc; ++i) process_file(argv[i]);
    export_trigrams("MHL1.txt");
    printf("Generated MHL1.txt\n");
    return 0;
}
