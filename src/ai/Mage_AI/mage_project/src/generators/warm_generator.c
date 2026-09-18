/* warm_generator.c - Warm Tier Low-Rank Generator
 * Generated: 2026-01-23 15:32:35
 */

#include "mage/types.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char* my_strdup_local(const char* s) {
	if (!s) return NULL;
	size_t n = strlen(s) + 1;
	char *r = malloc(n);
	if (!r) return NULL;
	memcpy(r, s, n);
	return r;
}

/* Minimal warm generator: sample tokens from a vocabulary file and produce
 * a short token sequence. This is a placeholder wiring that demonstrates
 * using the vocabulary produced by `python/tools/build_vocab.py`.
 * Real implementation should use low-rank reconstruction and numeric kernels.
 */

char* warm_generate_from_vocab(const char* prompt, const char* vocab_path, int max_tokens) {
	(void)prompt;
	if (!vocab_path) return NULL;
	FILE *f = fopen(vocab_path, "r");
	if (!f) return NULL;
	/* read top N tokens into memory */
	char line[1024];
	char **tokens = NULL;
	size_t tcap = 0;
	size_t tlen = 0;
	while (fgets(line,sizeof(line),f)) {
		char *p = strchr(line,'\n');
		if (p) *p='\0';
		if (line[0]=='\0') continue;
		/* parse JSON simple: find "token":"..." */
		char *tok = strstr(line, "\"token\"");
		if (!tok) continue;
		char *q = strchr(tok, ':');
		if (!q) continue;
		q++;
		while (*q && (*q==' ' || *q=='\t')) q++;
		if (*q == '"') q++;
		char *e = strchr(q,'\"');
		if (!e) continue;
		*e='\0';
		if (tlen+1 >= tcap) { size_t nc = tcap? tcap*2 : 64; char **n = realloc(tokens, nc*sizeof(char*)); if (!n) break; tokens = n; tcap = nc; }
		tokens[tlen++] = my_strdup_local(q);
		if (tlen >= 512) break; /* cap read tokens */
	}
	fclose(f);
	if (tlen==0) {
		free(tokens);
		return NULL;
	}
	/* naive sampling: pick first few tokens to form a phrase */
	int outlen = max_tokens>0?max_tokens:8;
	size_t cap = 256; char *out = malloc(cap); if (!out) { for (size_t i=0;i<tlen;i++) free(tokens[i]); free(tokens); return NULL; }
	out[0]='\0'; size_t used=0;
	for (int i=0;i<outlen; ++i) {
		const char *t = tokens[i % tlen]; size_t need = strlen(t)+2; if (used + need + 1 > cap) { cap *= 2; char *n = realloc(out, cap); if (!n) break; out = n; }
		if (i) { out[used++] = ' '; }
		strcpy(out+used, t); used += strlen(t);
	}
	out[used] = '\0';
	for (size_t i = 0; i < tlen; i++) free(tokens[i]);
	free(tokens);
	return out;
}

/* TODO: Implement SIMD-optimized low-rank factorization */
/* Ensure V is stored transposed for cache-friendly access */
