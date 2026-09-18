Mage API Surface
=================

Overview
--------
This document summarizes the public C API exported from the `mage` core
library (headers in `include/mage`). It describes ownership, threading,
error conventions, and the most commonly-used entry points.

Ownership & Conventions
-----------------------
- Returned pointers (e.g. `char*`, `manifest_t*`, `mmapped_index_t*`) are
  newly-allocated. The caller is responsible for freeing them via the
  corresponding close/free function (or `free()` when appropriate).
- Functions that return `int` return >=0 on success and negative values on
  error. Negative codes are `MAGE_ERR_*` (see `include/mage/api.h`).
- Call `mage_api_init()` once at process startup and `mage_api_shutdown()` at
  process exit to allow orderly teardown of background services (plugins,
  self-heal, logging).

Threading
---------
- APIs are safe to call from multiple threads provided each thread uses
  independent objects/handles. Global init should happen prior to multi-
  threaded use.

Key Functions
-------------
- `mage_api_init()` / `mage_api_shutdown()` — initialize/teardown runtime
  services.
- `mage_api_train_from_file(path)` — train or ingest data from a file.
- `mage_api_generate(prompt)` — high-level generate pipeline (parse + generate + finalize), returns JSON string (caller frees).
- `mage_api_mmapped_open(path)` / `mage_api_mmapped_get()` / `mage_api_mmapped_close()` — access dense vector stores.
- `mage_api_manifest_load(path)` / `mage_api_manifest_get()` / `mage_api_manifest_free()` — manifest helpers.
- `mage_api_build_vocab_from_file(input, out, min_count)` — produce `data/vocab/*.jsonl` vocabulary files from scraped data.

Examples
--------
- Initialize and generate:

```c
if (mage_api_init() != 0) { /* handle error */ }
char* out = mage_api_generate("Hello world");
if (out) { puts(out); free(out); }
mage_api_shutdown();
```

- Load a mmapped index and get a vector:

```c
mmapped_index_t* idx = mage_api_mmapped_open("data/vectors.bin");
if (idx) {
    uint32_t d = mage_api_mmapped_dim(idx);
    float* vec = malloc(sizeof(float) * d);
    if (mage_api_mmapped_get(idx, 0, vec) == 0) {
        // use vec
    }
    free(vec);
    mage_api_mmapped_close(idx);
}
```

Further reading
---------------
See `include/mage/api.h` for function declarations and `docs/` for subsystem
documentation.
