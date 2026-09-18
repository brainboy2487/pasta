# MAGE: Markov Answer Generation Engine

## Overview

The Markov Answer Generation Engine (MAGE) is a deterministic, CPU-first system for
efficient text vectorization and retrieval using stochastic tensor processes.

Generated: 2026-01-23 15:32:35

## Key Features

- **Deterministic inference** with fully reproducible outputs
- **Bounded latency** via Rule 42 (42-operation parse steps)
- **Tiered memory optimization** (Hot/Warm/Cold generators)
- **CPU-first performance** through lazy block contraction
- **Real-time retrieval** with memory-mapped indexing

## Architecture

MAGE replaces memory-bound dense tensor operations with compute-efficient lazy
generation using three tiers:

- **Hot Tier**: Dense storage (~0.66 ns/element) for high-weight slices
- **Warm Tier**: Low-rank factorization (~7.1 ns/element) for moderate-weight slices
- **Cold Tier**: Separable analytic functions (~38.7 ns/element) for low-weight slices

## Project Structure

```
mage_project/
├── src/              # C source files
│   ├── core/         # Core modules (TAP, contraction, cooccurrence)
│   ├── generators/   # Tier generators (Warm, Cold)
│   ├── io/           # I/O and memory-mapped indexing
│   └── utils/        # Utilities (MPHF, hashing, RNG)
├── include/          # Public headers
├── tests/            # Unit tests and benchmarks
├── python/           # Python utilities and FFI bindings
├── docs/             # Documentation
└── data/             # Data files and corpus
```

## Building

```bash
# Using Make
make all

# Using CMake
mkdir build && cd build
cmake ..
make

# Run tests
make test

# Run benchmarks
make benchmark
```

## Quick Start

```bash
# Vectorize a document corpus
./bin/mage_cli vectorize --input data/corpus/cleaned.jsonl --output data/vectors/

# Query the system
./bin/mage_cli query --vectors data/vectors/vectors.bin --prompt "your query here"
```

## Development Phases

The project follows a four-phase development roadmap:

**Phase 0 (Foundation)**: Heap-based TAP shortlist, Warm inner-loop optimization, separable Cold generator, and microbenchmark harness.

**Phase 1 (Tuning)**: Rank sweeps, float32 path implementation, block LRU cache, and quantization pipeline.

**Phase 2 (Integration)**: Deterministic multithreading, dynamic tier policy, TAP budget counters, and C ABI with Python FFI.

**Phase 3 (Hardening)**: SIMD intrinsics, embedded integer kernels, telemetry infrastructure, and cross-platform benchmarks.

## Documentation

See the `docs/` directory for detailed documentation including architectural specifications, API references, and tutorials.

## License

[Your license here]

## Contact

[Your contact information]

## Local CI

Run the included local CI helper which builds the project and runs unit tests:

```bash
ci/run_ci_locally.sh
```
