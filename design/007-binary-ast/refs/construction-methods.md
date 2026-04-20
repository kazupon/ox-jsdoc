# Binary AST Construction Approach Alternatives (Other Than the Adopted Approach)

`design/007-binary-ast/` adopts **Approach c-1** (parser writes Binary AST
directly into the arena, typed AST eliminated), but several alternatives
existed during the consideration process. This document records the approaches
that were not adopted, for future reference.

## Hierarchy of Design Decisions

The Rust → JS transfer approach for the JSDoc AST went through 3 layers of
design decisions before arriving at c-1:

```text
Layer 1: Transfer approach (how to cross the boundary)
  ├─ JSON path (current Phase 0, not adopted)
  ├─ oxc Raw Transfer (Philosophy A, not adopted)
  └─ tsgo-style Binary AST (Philosophy B, adopted) ────┐
                                                       │
Layer 2: Binary AST construction approach (Rust internal) │
  ├─ Approach a: parse + serialize in a separate pass     │ ← discussed after Layer 1 decision
  ├─ Approach b: typed AST + binary built in parallel during parse │
  ├─ Approach c-1: parse = encode (typed AST eliminated) ──┐
  └─ Approach c-2: parse → typed AST → binary writer       │
                                                       │
Layer 3: Implementation details inside c-1 (finalized in each sub-phase) │
  ├─ NodeList strategy, compat_mode representation, etc.   │
  └─ (See main documents for details)                      │
                                              Adopted
```

---

## Layer 1: Transfer Approach Alternatives

→ For details, see [`./js-rust-transfer.md`](./js-rust-transfer.md) and
[`./tsgo-vs-oxc-ast-transfer.md`](./tsgo-vs-oxc-ast-transfer.md).
Only the key points are recorded here.

### Option 1: JSON path (Phase 0, currently in operation)

**Overview**: On the Rust side, `serde_json::to_string` → pass through NAPI
string to JS → decode with `JSON.parse`.

**Pros**:

- Lowest implementation cost (existing serde_json ecosystem)
- Cross-platform / Cross-runtime / Cross-architecture (LE/BE agnostic)
- Works as-is even on WASM

**Cons**:

- `serde_json::to_string` (Rust side ~375 µs) and `JSON.parse` (JS side ~333 µs)
  occupy **85%** of the total (typescript-checker.ts measurement)
- String allocation for all fields puts pressure on the V8 heap
- All nodes are eagerly materialized; lazy access is not possible

**Reason not adopted**: Performance bottleneck. The JS binding becomes as slow as
comment-parser.

### Option 2: oxc Raw Transfer (Philosophy A)

**Overview**: Share the Rust `#[repr(C)]` struct memory layout directly with JS.
Build a bump allocator on a 6 GiB ArrayBuffer using `Allocator::from_raw_parts`,
and have Rust write directly into the JS buffer. No data transfer, serialization,
or deserialization is needed.

**Pros**:

- Zero data copy within the same process
- Maximum performance from parsing through JS consumption
- Proven in oxc, in stable operation

**Cons** (mismatched with ox-jsdoc requirements):

- **Requires 64-bit little-endian** (32-bit / BE not supported)
- Requires **Node ≥ 22.0.0** or **Deno ≥ 2.0.0**
- **Bun is not supported** (explicitly excluded in oxc's `supported.js`)
- **WASM not supported** (wasm32 linear memory cannot be 4 GiB-aligned; oxc also
  falls back to the JSON path when going through WASM)
- Requires reserving 6 GiB of virtual memory (with pitfalls such as Windows OOM,
  macOS alignment rejection, etc.)

**Reason not adopted**: Since **WASM support is a primary requirement** for
ox-jsdoc, Raw Transfer is fundamentally not adoptable.

### Option 3: tsgo-style Binary AST (Philosophy B, adopted)

**Overview**: A **designed offset-based binary format** that does not depend on
the Rust memory layout. The encoder writes into the arena in an independent
format, and the decoder (both Rust and JS) reads it via DataView.

**Pros**:

- WASM compatible (with LE fixed, common across all environments)
- High portability (full support for NAPI / WASM / IPC / file)
- Changes to the Rust-side memory layout do not affect the format
- No environment-specific JS code required (single decoder)

**Cons**:

- Encode/decode overhead (relative to Raw Transfer)
- Requires explicit specification documentation and version management
- Higher implementation cost than Raw Transfer or JSON

**Reason adopted**: The only choice that satisfies both ox-jsdoc's WASM support
requirement and long-term maintainability.

---

## Layer 2: Binary AST Construction Approach Alternatives

From here on, the choices for the internal construction approach **assume
Philosophy B (Binary AST) was adopted in Layer 1**. c-1 is the adopted approach;
the others were not adopted.

### Approach a: Separate-pass encoding (typed AST + JSON-equivalent two-stage)

```text
1. Parser builds the typed AST (on the arena)
2. In a separate pass, walk the typed AST → call binary writer functions → write binary into the arena
3. Pass the binary to JS
```

**Pros**:

- No modifications required to the existing parser (typed AST construction stays as-is)
- typed AST consumers (Rust-side walkers, validators, analyzers) work unmodified

**Cons**:

- Memory duplication (both typed AST and Binary AST held in the arena)
- Adds a walk pass (3 stages: parse + walk + encode)
- The typed AST → binary conversion cost is incurred immediately after parsing
- Inefficient pattern: "if we end up discarding the typed AST anyway, we should
  just write binary from the start"

**Reason not adopted**: Memory duplication and the additional walk pass hinder
the performance goal (5-10x improvement). Compatibility with typed AST consumers
can be covered more efficiently by Approach c-2, so Approach a is half-baked.

### Approach b: Parallel construction (parser writes typed AST and binary simultaneously, both retained)

```text
1. Parser builds the typed AST and simultaneously calls the binary writer (1 pass)
2. typed AST and binary AST exist in parallel on the arena
3. Pass the binary to JS. Rust-side walkers use the typed AST
```

**Pros**:

- Only one traversal needed (both can be written during parse)
- Maintains compatibility with typed AST consumers (Rust side)
- The JS side is accelerated via binary

**Cons**:

- Memory duplication (both typed AST and Binary AST held in the arena)
- Parser logic is duplicated (write calls for both typed AST and binary)
- The typed AST becomes "internal Rust-only purpose," incurring maintenance cost

**Reason not adopted**: Inferior to c-1 in both memory efficiency and
maintainability. If the Rust walker also goes through the lazy decoder, the
typed AST is unnecessary.

### Approach c-2: parser → typed AST → built-in binary writer (2-stage processing inside the parser)

```text
1. Parser builds the typed AST (on the arena, same as current)
2. At the end of parse(), walk the typed AST and write binary onto the arena
   via the binary writer
3. Return the binary byte sequence as the return value of parse() (typed AST also retained)
```

**Pros**:

- Small modifications to the parser internals (existing parser is mostly preserved,
  binary writer is added afterwards)
- Easy phased migration (e.g., binary path in Phase 1, typed AST removal in Phase 2)
- typed AST consumers (validators, analyzers) continue to work for the time being

**Cons**:

- Memory duplication is the same as Approach b
- Adds a walk pass (2 stages: parse → encode)
- Once we decide to "ultimately discard the typed AST," there is no longer a
  reason for the duplication

**Initially decided as c-2 → changed to c-1**:

> Initially c-2 was scheduled to be adopted (already finalized in the old version
> of `refs/binary-ast-batch-processing.md`). The reason was "minimize parser
> modification cost, leverage existing typed AST test assets."
>
> However, in subsequent consideration:
>
> - We decided that "the Rust-side walker would also go through the lazy decoder,"
>   eliminating the need for the typed AST
> - We shifted to a policy of "eliminating the typed AST" at the Phase 1.3 cutover
> - By eliminating memory duplication and the additional walk pass, c-2 → c-1
>   is expected to provide **about 1.3-1.5x speedup** (parser standalone time)
>
> For this reason, we **changed to Approach c-1 on 2026-04-19** (see
> `refs/binary-ast-draft-v2-deep-review.md`).

### Approach c-1: parser writes Binary AST directly into the arena (adopted)

```text
1. The parser writes directly into the arena in Binary AST format, without going
   through a typed AST (parse = encode, no additional pass)
2. The Binary AST on the arena is the single source of truth
3. The Rust-side walker also goes through the lazy decoder (Copy value types
   such as LazyJsdocBlock)
4. The Binary AST byte sequence is shared zero-copy with JS
```

**Pros**:

- Minimum memory (only one Binary AST, no typed AST)
- Minimum traversal (parse = encode, no additional walk pass)
- Maximum performance (5-10x improvement target expected to be achieved)
- Same lazy decoder pattern in Rust and JS → with code generation (Phase 4),
  both can be generated simultaneously from a single schema

**Cons**:

- Large parser modifications (existing typed AST construction code is changed
  to binary writer calls)
- Rewriting typed AST consumers (validators, analyzers) is mandatory
- Treated as a pre-1.0 stage breaking change

**Reason adopted**: ox-jsdoc is in the pre-1.0 stage with high tolerance for
breaking changes, and maximizing performance is the top priority. The Rust
walker is also fast enough with the lazy decoder (stack value type + Copy, no
Box allocation).

---

## Approach Comparison Table (Layer 2 only)

| Aspect                          | Approach a (separate pass)   | Approach b (parallel construction) | Approach c-2 (built-in walker)    | Approach c-1 (adopted)           |
| ------------------------------- | ---------------------------- | ---------------------------------- | --------------------------------- | -------------------------------- |
| **typed AST**                   | Retained (no plan to remove) | Retained (for Rust side)           | Retained → removed in later phase | **Removed**                      |
| **Memory**                      | Duplicated (large)           | Duplicated (large)                 | Duplicated (medium)               | **Minimum**                      |
| **Number of walk passes**       | 2 (parse + encode)           | 1 (parallel)                       | 2 (parse + encode)                | **1 (parse = encode)**           |
| **Parser modification**         | Not required                 | Small                              | Small                             | **Large**                        |
| **Existing tests**              | 100% functional              | 100% functional                    | 100% functional                   | Rewriting required               |
| **Rust walker**                 | typed AST                    | typed AST                          | typed AST → lazy later            | **lazy decoder**                 |
| **Performance (estimated)**     | Small improvement (~2-3x)    | Medium improvement (~3-5x)         | Medium improvement (~3-5x)        | **Maximum improvement (~5-10x)** |
| **Maintainability (long-term)** | Dual-retention cost          | Dual-retention cost                | Migration cost                    | **Simple**                       |

---

## Why c-1 (Summary)

1. **Performance maximization**: Eliminates memory duplication and additional
   walk pass, with the 5-10x improvement target expected to be achieved
2. **Maintainability**: Binary AST is the single source of truth, and code
   generation also produces both languages from a single schema
3. **Pre-1.0 stage**: High tolerance for breaking changes, allows rewriting of
   validators/analyzers
4. **Sufficiency of the lazy decoder**: The Rust-side walker is also Copy
   value-type stack-based lazy, as fast as or faster than the typed AST
   (no Box allocation)

---

## Related References

- [`./js-rust-transfer.md`](./js-rust-transfer.md) — Background of Layer 1 selection (JSON / Raw Transfer / Binary AST)
- [`./tsgo-vs-oxc-ast-transfer.md`](./tsgo-vs-oxc-ast-transfer.md) — Detailed comparison of Philosophy A vs B
- [`./tsgo/tsgo-binary-ast.md`](./tsgo/tsgo-binary-ast.md) — tsgo Binary AST (implementation example of Philosophy B)
- [`./benchmark-results.md`](./benchmark-results.md) — Measured bottleneck values of the JSON path (equivalent to Approach a)
- [`./binary-ast-draft-v2-deep-review.md`](./binary-ast-draft-v2-deep-review.md) — Review record at the time of the c-2 → c-1 change
