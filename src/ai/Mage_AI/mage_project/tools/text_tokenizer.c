#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <dirent.h>
#include <sys/stat.h>

#define MAX_TOKEN 128
#define HASH_SIZE 4096

// -------------------- Safe strdup replacement --------------------
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

// -------------------- Hash Table Structures --------------------

typedef struct NextNode {
    char *token;
    int count;
    struct NextNode *next;
} NextNode;

typedef struct MarkovNode {
    char *token;
    NextNode *next_list;
    struct MarkovNode *next;
} MarkovNode;

static MarkovNode *markov_table[HASH_SIZE];

// -------------------- Hash Helpers --------------------

unsigned int hash_str(const char *s) {
    unsigned int h = 5381;
    while (*s) h = ((h << 5) + h) + *s++;
    return h % HASH_SIZE;
}

MarkovNode *get_markov_node(const char *token) {
    unsigned int h = hash_str(token);
    MarkovNode *node = markov_table[h];

    while (node) {
        if (strcmp(node->token, token) == 0)
            return node;
        node = node->next;
    }

    // Create new node
    node = malloc(sizeof(MarkovNode));
    if (!node) {
        fprintf(stderr, "Out of memory allocating MarkovNode\n");
        exit(1);
    }

    node->token = xstrdup(token);
    node->next_list = NULL;
    node->next = markov_table[h];
    markov_table[h] = node;

    return node;
}

void add_transition(const char *current, const char *next) {
    MarkovNode *m = get_markov_node(current);

    NextNode *n = m->next_list;
    while (n) {
        if (strcmp(n->token, next) == 0) {
            n->count++;
            return;
        }
        n = n->next;
    }

    // Add new next-token
    NextNode *new_node = malloc(sizeof(NextNode));
    if (!new_node) {
        fprintf(stderr, "Out of memory allocating NextNode\n");
        exit(1);
    }

    new_node->token = xstrdup(next);
    new_node->count = 1;
    new_node->next = m->next_list;
    m->next_list = new_node;
}

// -------------------- Tokenizer --------------------

void normalize(char *s) {
    for (int i = 0; s[i]; i++) {
        if (!isalnum((unsigned char)s[i]) && s[i] != '\'')
            s[i] = ' ';
        else
            s[i] = tolower((unsigned char)s[i]);
    }
}

void process_file(const char *path) {
    FILE *f = fopen(path, "r");
    if (!f) return;

    char buf[4096];
    char prev[MAX_TOKEN] = {0};

    while (fgets(buf, sizeof(buf), f)) {
        normalize(buf);

        char *tok = strtok(buf, " \t\n");
        while (tok) {
            if (prev[0] != 0)
                add_transition(prev, tok);

            strncpy(prev, tok, MAX_TOKEN - 1);
            prev[MAX_TOKEN - 1] = '\0';

            tok = strtok(NULL, " \t\n");
        }
    }

    fclose(f);
}

// -------------------- Directory Walker --------------------

void walk(const char *path) {
    struct stat st;
    if (stat(path, &st) != 0)
        return;

    if (S_ISDIR(st.st_mode)) {
        DIR *d = opendir(path);
        if (!d) return;

        struct dirent *e;
        while ((e = readdir(d))) {
            if (e->d_name[0] == '.') continue;

            char full[1024];
            snprintf(full, sizeof(full), "%s/%s", path, e->d_name);
            walk(full);
        }

        closedir(d);
    } else {
        const char *ext = strrchr(path, '.');
        if (ext && strcmp(ext, ".txt") == 0)
            process_file(path);
    }
}

// -------------------- Export --------------------

void export_markov(const char *outfile) {
    FILE *f = fopen(outfile, "w");
    if (!f) {
        fprintf(stderr, "Failed to open %s for writing\n", outfile);
        return;
    }

    for (int i = 0; i < HASH_SIZE; i++) {
        MarkovNode *m = markov_table[i];
        while (m) {
            fprintf(f, "token:%s", m->token);

            NextNode *n = m->next_list;
            while (n) {
                fprintf(f, " -> %s:%d", n->token, n->count);
                n = n->next;
            }
            fprintf(f, "\n");

            m = m->next;
        }
    }

    fclose(f);
}

// -------------------- Main --------------------

int main(int argc, char **argv) {
    if (argc < 2) {
        printf("Usage: mage_markov <file_or_directory>\n");
        return 1;
    }

    walk(argv[1]);
    export_markov("M1_data.txt");

    printf("Generated M1_data.txt\n");
    return 0;
}
