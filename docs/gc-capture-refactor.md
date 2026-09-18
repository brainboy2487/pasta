GC-backed capture allocation
===========================

Problem
-------
During module load the current implementation suppresses DEF-time capture to avoid deep cloning of large nested values. This prevents exponential memory/CPU blowups but changes module-load capture semantics. A long-term solution is to store closure captures as GC-managed handles rather than cloning values.

Design overview
---------------
- Introduce a CaptureHandle type (u64) referencing a GC heap cell.
- When a DEF/lambda captures a variable, allocate a proxy on the GC heap and store the handle in the FunctionDef instead of cloning the value.
- The proxy object will hold either an index into an environment frame or a direct heap value; reads/writes through the handle will be supported by executor access primitives.
- Update GC tracing to mark proxies and referenced values.

Implementation steps
--------------------
1. Add CaptureHandle and helper API on GC (allocate_proxy, get_proxy, set_proxy).
2. Update ex_eval::FunctionDef creation sites to emit handles instead of cloned Values.
3. Update Function call path to deref handles when referencing captured variables.
4. Add unit tests that assert module-load snapshot semantics are preserved and that deep cloning no longer occurs (benchmarks).
5. Benchmark and iterate.

Tests
-----
- Reproduce earlier smoke-harness deep-clone scenario and assert runtime and memory usage reduced.
- Add language tests for module-level captures to ensure semantics are identical to earlier behaviour.

Next steps
----------
- Start by locating all capture creation sites and code paths that clone captures (ex_eval.rs, executor.rs). Prototype small handle-based allocation for simple lists/objects.
- Iterate and run full test harness after each change.
