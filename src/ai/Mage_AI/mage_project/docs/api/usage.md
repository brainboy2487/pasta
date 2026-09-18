**Mage C API — Usage Examples**

- **Init / Shutdown**: initialize runtime services (plugins, self-heal, logging) before calling other APIs.

```c
#include "mage/api.h"

int main(void) {
    if (mage_api_init() != 0) {
        fprintf(stderr, "mage_api_init failed\n");
        return 1;
    }
    /* ... call into API ... */
    mage_api_shutdown();
    return 0;
}
```

- **Train / Build Vocab**: call `mage_api_train_from_file()` to ingest text/JSONL and update vocabulary.

```c
int rc = mage_api_train_from_file("data/corpus/mydocs.jsonl");
if (rc < 0) {
    fprintf(stderr, "train error: %d\n", rc);
} else {
    printf("Added %d entries\n", rc);
}
```

- **Programmatic Vocab Generation**: use `mage_api_build_vocab_from_file()` to produce a JSONL vocab file.

```c
int n = mage_api_build_vocab_from_file("data/corpus/mydocs.jsonl", "data/vocab/generated_vocab.jsonl", 1);
if (n < 0) { /* handle error */ }
```

- **Generation (high-level)**: `mage_api_generate()` parses a prompt and returns a JSON string `{\"generated\": \"...\"}`.

```c
char* out = mage_api_generate("Summarize the latest notes");
if (out) {
    puts(out);
    free(out);
}
```

- **MMapped vectors**: open an index and fetch a vector.

```c
mmapped_index_t* idx = mage_api_mmapped_open("data/vectors.bin");
if (idx) {
    uint32_t d = mage_api_mmapped_dim(idx);
    float* vec = malloc(sizeof(float) * d);
    mage_api_mmapped_get(idx, 0, vec);
    mage_api_mmapped_close(idx);
    free(vec);
}
```

- **MPHF**: load and lookup keys (fast, O(1)).

```c
#include "mage/mphf.h"
mphf_t* m = mphf_load("data/mphf.bin");
if (m) {
    int idx = mphf_lookup(m, "hello");
    if (idx >= 0) printf("key index=%d\n", idx);
    mphf_destroy(m);
}
```

- **Tokenization**: obtain token array and free it when done.

```c
char** tokens = NULL; size_t count = 0;
if (mage_api_tokenize("Hello world", &tokens, &count) == 0) {
    for (size_t i=0;i<count;++i) puts(tokens[i]);
    mage_api_free_tokens(tokens, count);
}
```

Notes
- Error codes: functions returning `int` yield `>=0` on success and negative `MAGE_ERR_*` codes on failure.
- Ownership: functions that return pointers return newly-allocated objects unless documented otherwise; callers must free/close them.
- Threading: call `mage_api_init()` once before multi-threaded use.
