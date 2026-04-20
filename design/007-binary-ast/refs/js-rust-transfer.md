# JS-Rust AST Transfer Strategy

Investigation and policy for the JavaScript-Rust transfer method of the ox-jsdoc AST.

Note:

- "raw transfer" here refers to the **NAPI parser transport layer** in `refers/oxc`
- The core AST design itself and the JS transport mechanism should be considered separately

---

## Background

ox-jsdoc uses the JSDoc AST parsed on the Rust side from the JavaScript side as well.
The transfer overhead at this point becomes an issue.
The oxc project solves this problem with a mechanism called "raw transfer".

---

## How oxc raw transfer works

### Overview

A mechanism that, when passing AST from Rust to JavaScript, avoids ordinary JSON/serde-like serialization
and reconstructs on the JS side based on the Rust-side arena memory layout.

More precisely:

- Uses Rust's memory layout itself as the "transport format"
- The JS side reads the buffer using a pre-generated deserializer / lazy constructor
- Therefore this is closer to an "AST transport ABI" rather than "AST semantics"

### Architecture

```
JS side: Allocate a sufficiently large ArrayBuffer / Uint8Array
  ↓ Use `getBufferOffset()` to find a starting position aligned to a 4GiB boundary
  ↓ Pass the ArrayBuffer to Rust (NAPI)
Rust side: Use part of the buffer as backing memory for the arena via `Allocator::from_raw_parts`
  ↓ Parser runs as usual; AST nodes are written directly into the ArrayBuffer
  ↓ Metadata (offsets, etc.) is recorded at the end of the buffer
JS side: Code-generated deserializer reconstructs from Uint8Array / uint32 view
  ↓ For eager mode, builds plain JS objects; for lazy mode, builds wrappers/classes with getters
```

### Core technologies

| Element                          | Detail                                                                                                  |
| -------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `#[repr(C)]` + layout assertions | Fixes field order / size / align of AST type group; generated `assert_layouts` detects regressions      |
| Code generation                  | `tasks/ast_tools` generates JS eager deserializer / lazy constructors / constants / Rust-side constants |
| 4GiB alignment                   | Since the upper 32 bits of 64-bit pointers become common, JS can treat them as 32-bit offsets           |
| Lazy deserialization             | Getter/class-based reconstruction that materializes nodes only when actually accessed                   |
| Buffer pooling                   | `FinalizationRegistry` reuses buffers on GC                                                             |

### Deserialization modes on the JS side

**Eager**: The generated deserializer scans the buffer and builds ESTree-compliant plain JS objects. The buffer can be returned to the cache immediately after deserialization.

**Lazy**: AST nodes are constructed only when actually accessed. Uses `RawTransferData` / generated constructors / `NodeArray` to read on-demand via getters. Returns buffer to the cache via `dispose()` and `FinalizationRegistry`.

### Performance implications

- AST is not repacked into another format on the Rust side
- Eager mode is a transport path that speeds up JS object construction
- Lazy mode "reads only nodes that become necessary on the JS side", so it can significantly reduce current-thread work in cases where access is sparse

Caution:

- Claims about fixed multipliers in this note are limited to the range found in local primary sources within `refers/oxc`
- `napi/parser/src-js/raw-transfer/eager.js` notes that async eager has heavy current-thread deserialize work, which tends to cancel out the benefits of async parsing
- `napi/parser/src-js/raw-transfer/lazy.js` notes that lazy mode has very little current-thread deserialize work

### Constraints

- Supports little-endian only
- Reserves a 6GiB virtual memory region and uses the 4GiB-aligned 2GiB portion
- A special path that treats existing buffers as an arena via `Allocator::from_raw_parts`
- As a runtime requirement, at least the JS side requires a 64-bit little-endian raw-transfer-capable runtime

About runtime:

- `src-js/raw-transfer/supported.js` determines raw-transfer-capable runtimes as
  - Node >= 22
  - Deno >= 2
  - Bun is not supported
- `package.json`'s `engines.node` is `^20.19.0 || >=22.12.0`, but this is the support range of the entire package, not matching the runtime requirements of raw transfer itself

### Issues and constraints

#### 1. Memory model: 64-bit only

The most fundamental issue. 4GiB boundary alignment assumes 64-bit systems and is impossible on 32-bit.

- **WASM is wasm32 (32-bit linear memory)**, so a 4GiB-aligned region cannot be reserved
- The oxc backlog issue #197 also explicitly states "all the targets Oxc supports **except WASM**"
- **When used via WASM in browsers, raw transfer cannot be used**
- The wasm64/memory64 proposal exists, but SpiderMonkey benchmarks report 10-100%+ performance degradation, so it has not been adopted
- An alternative concept of using `WebAssembly.Memory` as the arena's backing store exists but is unimplemented

#### 2. Platform-specific issues

| Platform     | Issue                                                                                         |
| ------------ | --------------------------------------------------------------------------------------------- |
| Windows      | Virtual memory overcommit limit causes OOM panic when reserving 6GiB (issue #19395, #20331)   |
| macOS        | System allocator rejects 4GiB alignment. Workaround: reserve with 2GiB alignment and use half |
| Linux (slim) | OOM in environments where overcommit is disabled                                              |
| mimalloc     | Rejects high-alignment requests. Uses system allocator directly                               |

#### 3. bumpalo hack

- This is **a special path against the current `oxc_allocator::Allocator`**, not directly `bumpalo` in current Oxc
- `raw_transfer.rs` uses `Allocator::from_raw_parts` to construct the arena directly on the buffer
- Therefore, if ox-jsdoc takes this direction, "whether to expose allocator internals / fixed raw layout for transport" becomes a design issue

#### 4. Type layout not guaranteed

There exist types where `#[repr(C)]` cannot be applied:

- `Vec<T>` — Rust's standard layout guarantees alone are insufficient; the generator side has assumptions about field offsets
- `&str` — It is dangerous to treat the Rust language specification's ABI directly as a cross-language contract
- `Option<T>` / niche-bearing types — The generator side needs to understand the handling of payload / tag
- `NodeId` — Special-cased on the raw transfer generator side, normally excluded from transfer targets

#### 5. Other

- **Endianness**: little-endian only
- **Thread scaling**: Thread-count + 1 of 4GiB allocator reservation (~68GiB virtual address space for 16 threads)
- **Async overhead**: Async version is slower than sync version due to thread creation

Supplement:

- `napi/parser/src/lib.rs` and `src-js/index.js` recommend `parseSync` as the basis for single-file parsing
- Even when using raw transfer, "making a single parse async" does not necessarily make it significantly faster

### oxc's planned solution (Issue #20513: Revamp allocator)

- Replace dual allocator of standard + fixed-size with a unified allocator
- Reserve virtual memory directly via `mmap`/`VirtualAlloc` (avoiding OOM)
- Per-thread allocator pools within a shared 4GiB address space
- Separated arenas for strings and AST data (bidirectional bumping)
- Make all arenas raw-transfer-capable by default

### References

- [Faster passing ASTs from Rust to JS - Issue #2409](https://github.com/oxc-project/oxc/issues/2409)
- [feat(ast/estree): raw transfer (experimental) - PR #9516](https://github.com/oxc-project/oxc/pull/9516)
- [Revamp allocator - Issue #20513](https://github.com/oxc-project/oxc/issues/20513)
- [Custom arena allocator - Backlog Issue #197](https://github.com/oxc-project/backlog/issues/197)
- [Windows OOM - Issue #19395](https://github.com/oxc-project/oxc/issues/19395)
- [Ubuntu-slim OOM - Issue #20331](https://github.com/oxc-project/oxc/issues/20331)
- [Oxlint JS Plugins Preview (Oct 2025)](https://oxc.rs/blog/2025-10-09-oxlint-js-plugins.html)
- [Oxlint JS Plugins Alpha (Mar 2026)](https://oxc.rs/blog/2026-03-11-oxlint-js-plugins-alpha.html)
- [V8: Up to 4GB of memory in WebAssembly](https://v8.dev/blog/4gb-wasm-memory)
- [SpiderMonkey: Is Memory64 actually worth using?](https://spidermonkey.dev/blog/2025/01/15/is-memory64-actually-worth-using.html)

---

## Applicability of raw transfer to ox-jsdoc AST

### Status of prerequisite conditions

The ox-jsdoc AST (`design/ast.md`) is fairly close to oxc's AST design principles and is in a form that makes it easy to proceed to raw transfer.

| raw transfer prerequisite               | ox-jsdoc AST (design)         | Status           |
| --------------------------------------- | ----------------------------- | ---------------- |
| `#[repr(C)]` for all nodes              | Yes                           | Satisfied        |
| Arena-based memory (`'a`)               | Yes                           | Satisfied        |
| `&'a str` zero-copy strings             | Yes                           | Satisfied        |
| Keep representative enums small + `Box` | Yes                           | Mostly satisfied |
| Drop-prohibited                         | Yes (since arena-based types) | Satisfied        |

However:

- Even in `oxc`, raw transfer is not the AST itself but has a **dedicated transport layer**
- Therefore, for `ox-jsdoc` as well, "the AST is raw-transfer friendly" and "raw transfer should be implemented immediately" are separate judgments

### Issues when introducing raw transfer

1. **Code generation of the JS deserializer** — If both eager / lazy modes are to be done, a generator is almost essential
2. **Buffer management** — A transport layer including 4GiB alignment, buffer cache, dispose, and runtime detection needs to be built
3. **Allocator's raw path** — How to provide a transport-dedicated entry equivalent to `Allocator::from_raw_parts`
4. **Cost-effectiveness for the scale** — Since the JSDoc AST is smaller than the JS/TS AST, JSON may be sufficiently fast
5. **Node-only nature** — Raw transfer is at least not a common solution for browser/wasm

---

## Policy (Updated 2026-04-19: Migration to Binary AST adoption)

### Premise: Arena-based memory allocation

AST memory allocation **always uses an arena model**. This does not change regardless of the JS-Rust transfer method.

- Zero alloc/dealloc cost for individual nodes during parsing
- The arena itself is alloc/dealloc'd only at startup and shutdown
- Fast node traversal due to cache locality

### Evolution of the transfer method

The ox-jsdoc transfer method was considered in the following 3 stages:

| Stage                                    | Proposal                                      | Status                                                                                   |
| ---------------------------------------- | --------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Initial (~2026-04-10)                    | JSON-based transfer (serde_json + JSON.parse) | Implemented and currently in operation                                                   |
| Intermediate consideration (~2026-04-13) | oxc Raw Transfer                              | **Not adopted** (WASM-incompatible, 64-bit LE-only does not match ox-jsdoc requirements) |
| **Current (from 2026-04-19)**            | **tsgo-style Binary AST (approach c-1)**      | ✅ **Adoption decided**                                                                  |

### Options not adopted

#### Raw Transfer (oxc method) — Not adopted

oxc's Raw Transfer has the following constraints and does not match ox-jsdoc requirements:

- **Requires 64-bit little-endian** (32-bit / BE not allowed)
- Requires **Node ≥ 22.0.0** or **Deno ≥ 2.0.0**
- **Bun is not supported** (explicitly excluded in oxc `supported.js`)
- **WASM not allowed** (wasm32 linear memory cannot be 4 GiB aligned)

Since **WASM support is a primary requirement** for ox-jsdoc (both NAPI/WASM are primary use cases), Raw Transfer cannot be adopted.

#### JSON transfer — Not adopted (migrated to Binary AST)

JSON had low implementation cost, but migrated to Binary AST for the following reasons:

- `serde_json::to_string` (Rust) and `JSON.parse` (JS) account for the majority of round-trip cost
  (typescript-checker.ts benchmark: JSON path 829 µs / existing NAPI direct path 30 µs)
- Frequent string allocations occur, putting pressure on the JS-side V8 heap
- Since all nodes are eagerly materialized, there is much waste in use cases like ESLint plugins
  that reference only a portion

### Adopted Binary AST (approach c-1)

Designed an ox-jsdoc-specific format with reference to the tsgo Binary AST:

- **Approach c-1**: parser builds Binary AST directly on the arena (typed AST removed)
- **lazy decoder**: Both Rust and JS read Binary AST with the same lazy expansion pattern
- **NAPI**: Zero-copy sharing of the byte sequence on the arena via NAPI Buffer
- **WASM**: JS views the arena region with `new Uint8Array(wasm.memory.buffer, ofs, len)`
- The design specification is finalized in `design/binary-ast-draft.md` v2 (3000+ lines)

### Lessons learned

- The essence to learn from `oxc` is not "fixing on a transport ABI from the start" but
  "separating the core AST and transport layer so that they can be swapped later"
- As a result, by choosing Binary AST instead of Raw Transfer, the tsgo-style
  "designed offset-based format" can support all of NAPI/WASM/IPC
- ox-jsdoc achieves a good balance: faster than plain JSON, more portable than Raw Transfer,
  and smaller in scale than tsgo
