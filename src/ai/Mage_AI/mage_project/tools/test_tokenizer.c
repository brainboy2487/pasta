#include <stdio.h>
#include <stdlib.h>
#include "mage/tokenizer.h"

int main(void) {
    const char *cases[] = {
        "Hello, world!",
        "Caf\xc3\xa9 co\u0301mpose", /* intentionally mixed bytes: café */
        "—em dash — and – en dash",
        "Smart quotes: “Hello” ’there’",
        "Fullwidth: ＡＢＣ１２３",
        "Non\u00A0breaking\u00A0space",
        NULL
    };

    for (int i = 0; cases[i]; ++i) {
        printf("Case %d: %s\n", i+1, cases[i]);
        char **tokens = NULL; size_t n = 0;
        int rc = mage_tokenize(cases[i], &tokens, &n);
        if (rc != 0) { printf("  tokenize failed\n"); continue; }
        printf("  tokens (%zu):", n);
        for (size_t j = 0; j < n; ++j) printf(" [%s]", tokens[j]);
        printf("\n");
        mage_free_tokens(tokens, n);
    }
    return 0;
}
