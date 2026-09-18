# pipelines

This module implements the new pipeline subsystem for Pasta.

Files:
- `pipe_api.rs` — channel primitives and error types.
- `pipes.rs` — high-level Pipeline API and helpers.
- `single_p.rs` — single-threaded `|` implementation (cooperative streaming).
- `double_p.rs` — `||` implementation (isolated interpreter contexts).
- `async_p.rs` — `|&|` implementation (async worker pool integration).
- `shared_p.rs` — `|:|` implementation (shared-thread binding for scripts/modules).

Bootstrap notes:
- The bootstrap provides safe, well-documented skeletons and a mutex+condvar channel implementation.
- Replace TODOs with interpreter-specific VM context creation, pasta_async integration, and canvas safety primitives.

Suggested next steps:
1. Wire `single_p::run_pipe_single_threaded` into the parser for `|`.
2. Implement a small interpreter-level cooperative scheduler to avoid OS sleeps.
3. Implement VM context spawn for `double_p`.
4. Integrate `async_p` with `pasta_async` worker pool and add thread-safe canvas APIs.
