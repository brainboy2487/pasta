#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>

#define HASH_SIZE 8192
#define MAX_LINE 8192
#define MAX_TOKENS_PER_LINE 256

// -------------------- Safe strdup --------------------
static char *xstrdup(const char *s) {
    size_t len = strlen(s) + 1;
    char *p = malloc(len);
    if (!p) {
        fprintf(stderr, "Out of memory in xstrdup\n");
        exit(1);
    }
    memcpy(p, s, len);
    return p;
}

// -------------------- 3rd-order Markov structures --------------------

typedef struct NextNode3 {
    char *token;              // next token (3rd in trigram)
    int count;
    struct NextNode3 *next;
} NextNode3;

typedef struct TriNode {
    char *t1;                 // first token of state
    char *t2;                 // second token of state
    NextNode3 *next_list;     // list of possible 3rd tokens
    struct TriNode *next;
} TriNode;

static TriNode *tri_table[HASH_SIZE];

// -------------------- Hash helpers --------------------

static unsigned int hash_pair(const char *a, const char *b) {
    unsigned int h = 5381;
    const unsigned char *p;

    for (p = (const unsigned char *)a; *p; ++p)
        h = ((h << 5) + h) + *p;

    h = ((h << 5) + h) + '|';

    for (p = (const unsigned char *)b; *p; ++p)
        h = ((h << 5) + h) + *p;

    return h % HASH_SIZE;
}

static TriNode *get_trinode(const char *t1, const char *t2) {
    unsigned int h = hash_pair(t1, t2);
    TriNode *node = tri_table[h];

    while (node) {
        if (strcmp(node->t1, t1) == 0 && strcmp(node->t2, t2) == 0)
            return node;
        node = node->next;
    }

    node = malloc(sizeof(TriNode));
    if (!node) {
        fprintf(stderr, "Out of memory allocating TriNode\n");
        exit(1);
    }

    node->t1 = xstrdup(t1);
    node->t2 = xstrdup(t2);
    node->next_list = NULL;
    node->next = tri_table[h];
    tri_table[h] = node;

    return node;
}

static void add_trigram(const char *t1, const char *t2, const char *t3) {
    TriNode *tri = get_trinode(t1, t2);

    NextNode3 *n = tri->next_list;
    while (n) {
        if (strcmp(n->token, t3) == 0) {
            n->count++;
            return;
        }
        n = n->next;
    }

    n = malloc(sizeof(NextNode3));
    if (!n) {
        fprintf(stderr, "Out of memory allocating NextNode3\n");
        exit(1);
    }

    n->token = xstrdup(t3);
    n->count = 1;
    n->next = tri->next_list;
    tri->next_list = n;
}

// -------------------- Parsing M*_data.txt lines --------------------
// Expected format example:
//   token:maximum -> of:2 -> kutuzov:1
//
// We ignore the numeric counts in the M* files and reconstruct a token
// sequence from the arrow chain:
//   [maximum, of, kutuzov]
// Then we slide a window of size 3 to build trigrams:
//   (maximum, of) -> kutuzov

static void process_line(char *line) {
    char *tokens[MAX_TOKENS_PER_LINE];
    int ntok = 0;

    for (int i = 0; i < MAX_TOKENS_PER_LINE; i++)
        tokens[i] = NULL;

    char *p = strstr(line, "token:");
    if (!p)
        return;

    p += 6; // skip "token:"
    while (*p == ' ' || *p == '\t')
        p++;

    // First token: read until space or '->' or end
    char buf[256];
    int blen = 0;

    while (*p && !isspace((unsigned char)*p) && *p != '-' && *p != '\n') {
        buf[blen++] = *p++;
        if (blen >= (int)sizeof(buf) - 1)
            break;
    }
    buf[blen] = '\0';

    if (blen == 0)
        return;

    tokens[ntok++] = xstrdup(buf);

    // Now parse each "-> token:count" segment
    while ((p = strstr(p, "->")) != NULL) {
        p += 2; // skip "->"
        while (*p == ' ' || *p == '\t')
            p++;

        if (!*p || *p == '\n')
            break;

        blen = 0;
        while (*p && *p != ':' && !isspace((unsigned char)*p) && *p != '\n') {
            buf[blen++] = *p++;
            if (blen >= (int)sizeof(buf) - 1)
                break;
        }
        buf[blen] = '\0';

        if (blen == 0)
            break;

        if (ntok < MAX_TOKENS_PER_LINE)
            tokens[ntok++] = xstrdup(buf);

        // Skip the ":count" part if present
        while (*p && *p != '\n')
            p++;
    }

    // Build trigrams from token sequence
    if (ntok >= 3) {
        for (int i = 0; i + 2 < ntok; i++) {
            add_trigram(tokens[i], tokens[i + 1], tokens[i + 2]);
        }
    }

    // Free temporary token copies (table has its own copies)
    for (int i = 0; i < ntok; i++) {
        free(tokens[i]);
    }
}

// -------------------- Process input files --------------------

static void process_file(const char *path) {
    FILE *f = fopen(path, "r");
    if (!f) {
        fprintf(stderr, "Warning: could not open %s\n", path);
        return;
    }

    char line[MAX_LINE];
    while (fgets(line, sizeof(line), f)) {
        process_line(line);
    }

    fclose(f);
}

// -------------------- Export higher-order table --------------------
// Format:
//   token:t1,t2 -> next1:count -> next2:count ...

static void export_trigrams(const char *outfile) {
    FILE *f = fopen(outfile, "w");
    if (!f) {
        fprintf(stderr, "Failed to open %s for writing\n", outfile);
        return;
    }

    for (int i = 0; i < HASH_SIZE; i++) {
        TriNode *tri = tri_table[i];
        while (tri) {
            fprintf(f, "token:%s,%s", tri->t1, tri->t2);

            NextNode3 *n = tri->next_list;
            while (n) {
                fprintf(f, " -> %s:%d", n->token, n->count);
                n = n->next;
            }
            fprintf(f, "\n");

            tri = tri->next;
        }
    }

    fclose(f);
}

// -------------------- Main --------------------
// Usage:
//   ./markov3_from_M M1_data.txt M2_data.txt ... M10_data.txt
// Output:
//   MHL1.txt

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr,
                "Usage: %s <M_data_file1> [M_data_file2 ...]\n"
                "Example: %s M1_data.txt M2_data.txt ... M10_data.txt\n",
                argv[0], argv[0]);
        return 1;
    }

    for (int i = 1; i < argc; i++) {
        process_file(argv[i]);
    }

    export_trigrams("MHL1.txt");
    printf("Generated MHL1.txt (3rd-order Markov from M*_data.txt)\n");

    return 0;
}
