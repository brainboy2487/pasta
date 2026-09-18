# MAGE Layered Architecture

## Overview
This document defines a clean layered architecture for MAGE and a stable public header/API surface for each layer. The goal is to keep module boundaries clear, ensure determinism, and make it easy to wire the end-to-end path: prompt → parse (TAP) → shortlist → retrieval → generation → coherence gate → response.

Core principles
- Strong layer separation (Presentation ↔ API ↔ Application Core ↔ Retrieval/Storage ↔ Ops).
- Single public surface for adapters: `mage_api` (C) and a small Python FFI.
- Determinism: fixed RNG seeds, fixed reduction ordering, and golden parity tests.
- Small, testable steps: one PR per subtask; run parity tests after changes.

## Layers

1. Presentation
- Purpose: adapters for human or automated users.
- Components: CLI (`src/mage_cli.c`), GUI (`src/gui/*`), API server (`services/api/server.py`), Python FFI (`python/mage/ffi.py`).
- Responsibility: accept requests, perform input validation/auth, call `mage_api_*` functions, and render responses.

2. API / Application Controller
- Purpose: stable, thin middle layer exposing operations for Presentation.
- Components: `src/mage_api.c`, `include/mage/api.h`.
- Responsibility: provide consistent functions that call into Application Core and Retrieval/Storage without exposing internals. Examples: `mage_api_init`, `mage_api_train_from_file`, `mage_api_mmapped_open`, `mage_api_mmapped_get`, `mage_api_mmapped_dim`, `mage_api_finalize_reply`, `mage_api_contract_*`, `mage_api_vectorize`.

3. Application Core
- Purpose: TAP parser, TAP shortlist, TAP budget enforcement, generators, contract, RP.
- Components: `src/core/*` (`tap.c`, `rp.c`, `contract.c`, `cooc.c`, `separable_generator.c`, forwardfeed, etc.) and headers in `include/mage/*.h`.
- Responsibility: deterministic prompt parsing, shortlist selection, block contraction, generator kernels. No direct IO or presentation concerns. All IO must be through `src/io/*` or API layer.

4. Retrieval & Storage
- Purpose: persistent storage and fast read access to vector data and manifests; deterministic retrieval adapters.
- Components: `src/io/*` (`io.c`, `mmappedindex.c`), `tools/write_vectors.c`, `data/` artifacts (`vectors.bin`, `manifest.jsonl`, `mphf.bin`).
- Responsibility: read/write vectors, mmapped index, manifest parsing, MPHF loading/lookup. Expose stable C APIs for `mmapped_index_open`, `mmapped_index_get`, `mmapped_index_dim`, and manifest readers.

5. Ops / Testing / Bench
- Purpose: developers' tools, benchmarks, and CI.
- Components: `tests/`, `tools/`, `bench/` (planned), `services/api/openapi.yaml`.
- Responsibility: unit tests, Python golden tests, microbench harnesses, CI scripts.

## Stable Header & API Surface
These headers are the authoritative public surface for each layer. Keep these headers stable and minimal; implementation files should include only the headers they need and not leak internals across layers.

- Presentation: no public header (adapters use `include/mage/api.h`).

- API controller
  - `include/mage/api.h`
    - mage_api_init(void)
    - mage_api_shutdown(void)
    - mage_api_print_help_all(void)
    - mage_api_train_from_file(const char*)
    - mage_api_finalize_reply(const char*) -> char*
    - mage_api_contract_set_cache(size_t)
    - mage_api_contract_flush(void)
    - mage_api_contract_precompute(...)
    - mage_api_mmapped_open(const char*) -> mmapped_index_t*
    - mage_api_mmapped_get(mmapped_index_t*, uint32_t, float*)
    - mage_api_mmapped_close(mmapped_index_t*)
    - mage_api_mmapped_dim(mmapped_index_t*) -> uint32_t

- Storage / IO
  - `include/mage/io.h`
    - write_vectors_bin(const char*, const float*, uint32_t n, uint32_t d)
    - read_vectors_bin(const char*, float** out, uint32_t* n, uint32_t* d)
    - io_mmapped_open(const char*) -> mmapped_index_t*
    - io_mmapped_get(mmapped_index_t*, uint32_t, float*)
    - io_mmapped_close(mmapped_index_t*)
    - io_mmapped_dim(mmapped_index_t*) -> uint32_t
  - `include/mage/mmappedindex.h`
    - mmapped_index_open(const char* path, size_t vector_dim, size_t vector_size)
    - mmapped_index_close(mmapped_index_t*)
    - mmapped_index_get(const mmapped_index_t*, size_t, float*)
    - mmapped_index_dim(const mmapped_index_t*) -> uint32_t
    - FFI wrappers: mage_mmapped_index_open/close/get

- Core modules (stable headers, minimal public surface)
  - `include/mage/parser.h`
    - mage_parse_prompt(const char*) -> char*
    - mage_print_help_all(void)
    - mage_train_from_file(const char*) -> int
    - mage_finalize_reply(const char*) -> char*
  - `include/mage/rp.h`
    - rp_project(...)
  - `include/mage/tap.h`
    - tap_context_create/destroy, tap_parse APIs
  - `include/mage/contract.h`, `include/mage/cooc.h`, `include/mage/mphf.h`
    - expose necessary functions for block contraction, cooccurrence access, and mphf lookup.

- Utilities
  - `include/mage/types.h`, `include/mage/tokenizer.h`, `include/mage/xorshift.h` etc.

## Data formats
- `vectors.bin`:
  - header: uint32_t n, uint32_t d
  - payload: n * d float32 (row-major)
- `manifest.jsonl`:
  - JSONL where each line maps: {"id":..., "offset": <vector_index>, ...}
- `mphf.bin`:
  - serialized MPHF built offline by `tools/gen_mphf.c`
- `data/vocab/generated_vocab.jsonl`:
  - dynamic vocab entries created by trainer; validated by coherence checker before being loaded/written.

## OpenAPI / Endpoints (minimal shapes)
- POST /vectorize
  - body: {"text": string}
  - resp: {"vector": [float], "version":"v1"}
- POST /retrieve
  - body: {"vector": [float], "k": int}
  - resp: {"results": [{"id":..., "score":..., "meta": {...}}]}
- POST /generate
  - body: {"prompt": string}
  - resp: {"generated": string}
- POST /tap/parse
  - body: {"text": string}
  - resp: {"shortlist": [...], "budget": int, "trace": [...]}
- GET /admin/indices
  - resp: {"indices": [...]}

## Determinism & Testing
- Enforce deterministic RNG seeds (xorshift32) in `rp`, TAP, and any sampling.
- Fixed reduction trees and 64-bit accumulators for multi-threaded accumulation.
- Golden Python tests (e.g., `python/tests/test_rp_golden.py`) must be run after changes to `rp.c`.

## Acceptance Criteria (per major feature)
- All public headers unchanged unless API bump requested.
- `mage_api` is the only entrypoint the Presentation layer depends on.
- Core modules compile into `lib/libmage.so` and unit tests pass.
- Data writers/readers handle malformed files gracefully and tests exist (we have `tools/write_vectors.c` and `tests/unit/test_vectors_writer.c`).

## Combined TODO (summary pulled from project plan and attachments)
See the tracked todo list for granular tasks; high-priority items include:
- Finalize API OpenAPI and wire server to `mage_api`.
- Implement retrieval adapter and `/retrieve` using mmapped index and manifest.
- Optimize `rp_project` with parity-preserving improvements.
- Complete TAP internals and MPHF integration for bounded Rule-42 parse.
- Add microbench harnesses for mmapped lookup and generator kernels.
- Finalize Python FFI and expand golden tests.
- Add CI gating for parity and microbench stability.

---

For the stable header list and the detailed tracked todo items, see the project tracker (TODO list). Commit this file into `docs/architecture/layers.md` as the authoritative architecture reference.
