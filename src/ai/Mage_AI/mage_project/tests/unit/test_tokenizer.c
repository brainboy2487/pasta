#include <stdio.h>
#include <string.h>
#include "mage/tokenizer.h"

static int compare_tokens(char **got, size_t got_n, const char *expected[], size_t exp_n) {
    if (got_n != exp_n) return 1;
    for (size_t i = 0; i < exp_n; ++i) {
        if (strcmp(got[i], expected[i]) != 0) return 1;
    }
    return 0;
}

int main(void) {
    struct {
        const char *text;
        const char *expected[8];
        size_t exp_n;
    } cases[] = {
        { "Hello, world!", { "hello", "world" }, 2 },
        { "Caf\xc3\xa9", { "café" }, 1 },
        { "—em dash — and – en dash", { "-em", "dash", "-", "and", "-", "en", "dash" }, 7 },
        { "Smart quotes: “Hello” ’there’", { "smart", "quotes", "hello", "there" }, 4 },
        { NULL, {NULL}, 0 }
    };

    int failures = 0;
    for (int i = 0; cases[i].text != NULL; ++i) {
        char **tokens = NULL; size_t n = 0;
        int rc = mage_tokenize(cases[i].text, &tokens, &n);
        if (rc != 0) { printf("Case %d: tokenize returned %d\n", i, rc); failures++; continue; }
        int cmp = compare_tokens(tokens, n, cases[i].expected, cases[i].exp_n);
        if (cmp != 0) {
            printf("Case %d FAILED. got (%zu):", i, n);
            for (size_t j = 0; j < n; ++j) printf(" [%s]", tokens[j]);
            printf(" expected (%zu):", cases[i].exp_n);
            for (size_t j = 0; j < cases[i].exp_n; ++j) printf(" [%s]", cases[i].expected[j]);
            printf("\n");
            failures++;
        } else {
            printf("Case %d passed.\n", i);
        }
        mage_free_tokens(tokens, n);
    }

    return failures == 0 ? 0 : 2;
}
