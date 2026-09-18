# Pasta Compiler ABI

This document defines the first stable runtime boundary for compiled Pasta artifacts.

## Goals

- Support the long-term self-hosting roadmap without exposing interpreter-only internals as ABI commitments.
- Keep scalar values cheap to pass.
- Keep heap-like/dynamic values runtime-owned so compiled code can grow feature support without freezing raw in-memory layouts too early.

## ABI scope

The stable compiled ABI currently covers **user-visible language values** only:

- `Number`
- `String`
- `Bool`
- `List`
- `Dict`
- `None`
- `FamilyNode`
- `Pointer`
- `Tensor`

The following interpreter-only/internal variants are **not** part of the stable compiled ABI:

- `Lambda`
- `LazyImport`
- `Builtin`
- `Pending`
- `Heap`

## Representation strategy

### Inline scalars

These values are passed inline inside `PValue`:

- `None`
- `Bool`
- `Number`

### Runtime-owned handles

These values cross the ABI as **opaque runtime-managed handles**:

- `String`
- `List`
- `Dict`
- `Tensor`

Compiled code should treat these as opaque IDs only. Construction, lookup, mutation, retain/release, and destruction happen through runtime ABI calls rather than direct memory layout access.

### Explicit runtime IDs

These values remain explicit stable IDs in the ABI:

- `Pointer` -> `u64`
- `FamilyNode` -> `{ id: u64, mutable_flag: u8 }`

This aligns with the current runtime, where pointer IDs and family IDs are already stable-width identifiers.

## Core C-compatible layout

```c
enum PValueTag : uint32_t {
    PV_NONE = 0,
    PV_BOOL = 1,
    PV_NUMBER = 2,
    PV_HANDLE = 3,
    PV_FAMILY_NODE = 4,
    PV_POINTER = 5,
};

enum PHandleKind : uint32_t {
    PH_STRING = 1,
    PH_LIST = 2,
    PH_DICT = 3,
    PH_TENSOR = 4,
};

struct PFamilyNodeValue {
    uint64_t id;
    uint8_t mutable_flag;
    uint8_t reserved[7];
};

union PValueData {
    uint8_t boolean;
    double number;
    uint64_t handle_id;
    struct PFamilyNodeValue family_node;
    uint64_t pointer_id;
};

struct PValue {
    uint32_t tag;
    uint32_t handle_kind; /* meaningful only when tag == PV_HANDLE */
    union PValueData data;
};
```

Current Rust definitions live in `src/runtime/abi.rs`.

## Ownership model

### Scalars

Scalars are copied by value.

### Handle values

Handle values are owned by the runtime. Compiled code may receive, pass, and return them, but must not assume anything about internal memory layout.

The next ABI layer should add runtime entrypoints along these lines:

- `pasta_value_retain(PValue)`
- `pasta_value_release(PValue)`
- `pasta_string_new(...)`
- `pasta_list_new(...)`
- `pasta_dict_new(...)`
- `pasta_tensor_new(...)`

Those APIs now exist as the first exported helper slice:

- `pasta_value_retain(PValue) -> PAbiStatus`
- `pasta_value_release(PValue) -> PAbiStatus`
- `pasta_string_new(bytes, len, out_value) -> PAbiStatus`
- `pasta_string_view(value, out_bytes, out_len) -> PAbiStatus`
- `pasta_list_new(values, len, out_value) -> PAbiStatus`
- `pasta_dict_new(entries, len, out_value) -> PAbiStatus`
- `pasta_tensor_new(shape, dtype, data, device, out_value) -> PAbiStatus`

These helpers use **status-code + out-pointer** calling style instead of silent sentinel returns, so invalid inputs can be surfaced explicitly at the ABI boundary.

## Calling convention

Compiled shared-module exports now use the first stabilized call contract:

- ABI version: `1`
- Call ABI label: `pasta.module.v1`
- Ownership model: `retain_release_refcounted`
- Calling shape: `export(argc, argv, out_value) -> PAbiStatus`

Where:

- `argc` is the exact argument count for the export,
- `argv` points to a contiguous `PValue[argc]` array,
- `out_value` points to writable storage for the returned `PValue`,
- the return value is a `PAbiStatus` code and **must** be checked before reading `out_value`.

For `pasta.module.v1`, the status codes used by compiled-module wrappers are:

- `Ok`
- `NullOut`
- `NullInput`
- `InvalidUtf8`
- `InvalidHandle`
- `HandleKindMismatch`
- `InvalidArgument`
- `UnsupportedValue`
- `ArityMismatch`
- `TypeMismatch`

`ArityMismatch` is used when `argc` does not match the export signature. `TypeMismatch` is used when an argument `PValue` tag/handle kind does not match the export signature. Helper-originated failures such as `pasta_string_view` continue to propagate their helper status codes directly.

## Module boundary direction

This ABI is intended to support:

1. compiled executables,
2. compiled shared modules,
3. interpreter <-> compiled module calls,
4. eventual Pasta-written compiler/runtime layers.

The first stabilized shared-module contract now uses predictable exported symbols:

- `pasta_mod_<module>_manifest_json() -> const char*`
- `pasta_mod_<module>_init() -> PAbiStatus`
- `pasta_mod_<module>_call_<export>(argc, argv, out_value) -> PAbiStatus`

The manifest now describes:

- module name,
- ABI version,
- call ABI label,
- handle ownership model,
- symbol visibility policy,
- exported function names,
- arity,
- parameter types,
- return type.

Export signatures may now use the opaque type name `value`. In `pasta.module.v1`, `value` means a full ABI `PValue` crossing the boundary with the same retain/release ownership rules as other runtime-managed values. Compiled shared-module code can currently **pass these opaque values through locals and nested direct compiled calls**, but it still cannot inspect or mutate their internal structure directly.

The host interpreter loads the shared library, validates that manifest contract, calls `init`, then binds each exported function through ABI value conversion helpers.

## Symbol visibility rules

For compiled shared modules, the public surface is intentionally narrow:

- exported symbols: manifest function, init function, and per-export call wrappers,
- hidden symbols: compiled function implementations and module globals.

In other words, compiled user functions are callable from outside the shared library only through their documented `pasta_mod_<module>_call_<export>` ABI wrappers. This keeps the low-level module boundary stable even if internal LLVM symbol naming changes later.

## Current limitations

This ABI document now defines the value boundary plus the first shared-module call surface, but the broader dynamic runtime contract is still incomplete.

Still to define:

- exported globals/constants for shared modules,
- richer handle inspection/mutation APIs beyond creation, string view, and retain/release,
- direct compiled introspection/mutation of opaque dynamic values beyond pass-through semantics.
