# binary-ast-draft.md v2 — Maximum-effort Deep Review (2026-04-19)

Review target: `design/binary-ast-draft.md` (2965 lines before fixes → 3109 lines after fixes)

The previous reviews (`binary-ast-draft-v2-review.md`, `binary-ast-draft-v2-final-review.md`)
already produced 20+ fixes, but a maximum-effort full re-read uncovered **8 new
inconsistencies**, all of which have been **fully resolved** (2026-04-19).

## Status: All findings resolved

| Category     | Count | State |
| ------------ | ----- | ----- |
| Critical     | 3     | Fixed |
| Should Fix   | 3     | Fixed |
| Nice to Have | 2     | Fixed |

---

## Critical — design contradictions that block implementation decisions

### #1: JsdocBlock — contradiction between Node Data type and child storage

**Location**: L1035-1037 (within the Children type subsection) vs L1052 (Extended type list)

**Contradiction**:

- **L1052** (Extended type 0b10):

  ```
  Used by JsdocBlock, JsdocTag, JsdocInlineTag, JsdocParameterName, etc.
  ```

- **L1035-1037** (within the Children type 0b00 subsection):
  ```
  Example: child properties of JsdocBlock:
  - bit0 = description_lines, bit1 = tags, bit2 = inline_tags
  ```

JsdocBlock appears as **an example for both the Children type and the Extended type**.
Since there is only one Node Data slot, both cannot hold simultaneously.
**Implementers cannot decide where to place the Children bitmask**.

**Real impact**: the **empty-array skip optimization** for descriptionLines/tags/inlineTags
(L1203-1238) requires the bitmask, but its placement is undefined.
JsdocTag alone explicitly states "Extended Data byte 0" at L776-791,
but JsdocBlock has no such explicit specification.

**Proposed fix**:

- Change the example at L1035 from "JsdocBlock" (inappropriate because it is Extended type)
  to a pure Children-type node (e.g., the existing TypeFunction, or JsdocBorrowsTagBody).
- Add "byte 0: Children bitmask (u8)" to the JsdocBlock Extended Data layout at L766
  and update the size to **17 bytes (basic) / 41 bytes (compat)**.
- If alignment is taken into account, use byte 0 = bitmask + byte 1 = padding,
  giving **18 bytes (basic) / 42 bytes (compat)**.

---

### #2: TypeMethodSignature — contradictory Common Data bit assignments

**Location**: L919-921 (TypeMethodSignature detailed spec) vs L1074 (Common Data table)

| Location                  | bit0                  | bit1          |
| ------------------------- | --------------------- | ------------- |
| L919-921 (detailed spec)  | quote_present         | quote_style   |
| L1074 (Common Data table) | quote (Single/Double) | quote_present |

**The meanings of bit 0/1 are completely reversed**. A decoder can only implement one of them correctly.

**Proposed fix**:
L919-921 is more detailed and clearer in its explanation
(`only meaningful when bit0 = 1`), so treat that as authoritative.
Split the TypeMethodSignature row at L1074 into its own row stating
`bit0 = quote_present, bit1 = quote_style`.

```
| `TypeStringValue` / `TypeProperty` | bit0 = quote (Single/Double), bit1 = quote_present |
| `TypeMethodSignature` | bit0 = quote_present, bit1 = quote_style |
```

---

### #3: TypeMethodSignature — Children bitmask placement is undefined

**Location**: L913-928 (TypeMethodSignature details)

**Problem**:
L923-927 states "represented as a Children bitmask" with "bit0 = parameters, bit1 = return_type,
bit2 = type_parameters", but **the physical placement of the bitmask is not specified**.

- Extended Data is 2 bytes (only the name string index, L917).
- Common Data already uses 2 bits (quote_present, quote_style) → 4 bits remain.

**Options**:

- (a) Place the bitmask in Common Data bits 2-4 (3 bits used, bit 5 reserved).
- (b) Add 1 byte to Extended Data and place the bitmask in byte 2 (breaks alignment).

**Proposed fix**: Adopt (a) and expand L919-921 as follows:

```
Common Data:
  bit0 = quote_present
  bit1 = quote_style
  bit2 = has_parameters    (NodeList if non-empty, omitted if empty)
  bit3 = has_type_parameters (same as above)
  (return_type is required, so no bit is needed; always present)
```

---

## Should Fix — explanations that are insufficient or could mislead

### #4: Incorrect line-number reference at L876

**Location**: L876

```
Note: every string index below is a **u16** (per the Extended Data convention, lines 707-722).
```

The actual "string index is u16" convention is at **L744-753**
(L707-722 covers the string table and Diagnostics section).

**Proposed fix**: Correct to "lines 744-753", or change the reference to point to the
`#### string index is u16 (2 bytes)` subsection of the `### Extended Data section`.

---

### #5: JsdocGenericTagBody — Node Data type and storage of `description?` are undefined

**Location**: L326, L1067, L1480 (related items are scattered)

**Problem**:

- L326: fields `type_source?, value?, separator?, description?`
- L1067: Common Data bit0 = has_dash_separator
- L1480: visitor key `['typeSource', 'value']`
- → typeSource / value are child nodes, separator is packed into Common Data,
  but the **storage location of description? is unclear**.
- L765 Extended Data table does not include JsdocGenericTagBody.

**Options**:

- (a) Store in Extended Data (description string index u16).
- (b) Store as a child node such as JsdocText.

**Proposed fix**: Adopt (a) and add JsdocGenericTagBody to the L765 table:

```
| `JsdocGenericTagBody` | 4 bytes | description string index (u16) + Children bitmask (u8) + reserved (u8) |
```

Also document the bit assignments of the Children bitmask (bit0 = typeSource, bit1 = value).

---

### #6: Consistency of the Phase 1.0a-d test schedule

**Location**: L2189 (test schedule table)

**Problem**:

- L2189: 1.0a-d adds "encoder tests (#2), snapshot (#12)".
- L2667-2680: Phase 1.0a-d is a **skeleton phase containing only `unimplemented!()` stubs**.

Encoder tests and hex-dump snapshots are meaningless while the encoder is unimplemented.

**Proposed fix**: Split the schedule as follows:

```
| 1.0a-d | unit tests (#1) only (verify type definitions/consts, confirm skeleton builds) |
| 1.1a   | add encoder tests (#2), snapshot (#12) (alongside the actual encoder implementation) |
| 1.1b   | add decoder tests (#3), memory safety (#10) |
| 1.1c   | add visitor tests (#9) |
| 1.1d   | add JS-side tests (#6) |
| 1.2a   | (parser implementation; no new tests added — regression covered by existing tests) |
| 1.2b-c | roundtrip (#4), compatibility tests (#5), edge cases (#8), cross-binding (#7) |
| 1.2d   | performance tests (#13) |
| Pre-1.3 | fuzzing (#11) |
```

---

## Nice to Have

### #7: Inconsistent quote-encoding styles

**Location**: L1074, L1076, L1077 (Common Data table)

**Problem**:

- TypeStringValue / Property: `bit0 = quote(Single/Double), bit1 = quote_present`
  (presence + style separated form, 2 bits)
- TypeObjectField (L1077): `bits[2:3] = quote` (combined 3-state form, 2 bits)
- TypeSpecialNamePath (L1076): `bit2-3 = quote` (combined 3-state form, 2 bits)

All represent the same `Option<QuoteStyle>` but use different encodings.
(Combined 3-state uses None=0, Single=1, Double=2 in 2 bits;
presence + style separated uses quote_present=0/1 + style=0/1 in 2 bits — same bit count.)

**Proposed fix**: Unify all nodes on the combined 3-state form (None=0, Single=1, Double=2)
to make the encoding style consistent and simplify decoder implementation.

---

### #8: A matrix table summarizing the Node Data type of every node would be useful

**Location**: described in prose (L839-868, L1030-1054, L1063-1079)

**Problem**:
The current text explains "this is Children type" / "this is Extended type" in prose,
but a single table covering **all 60 kinds × Node Data type × Common Data usage × Extended Data size**
would let decoder implementers grasp every node at a glance.

**Proposed fix**: Add a new subsection:

```
### Node matrix

| Kind | Node name | Node Data type | Common Data | Extended Data size |
|---|---|---|---|---|
| 0x01 | JsdocBlock | Extended | (unused) | 17 / 41 bytes |
| 0x02 | JsdocDescriptionLine | Children or Extended (compat) | - | 0 / 8 bytes |
| 0x03 | JsdocTag | Extended | bit0 = optional | 8 / 22 bytes |
| ... (all 60 kinds) |
```

---

## Confirmation: items that are consistent

- **NodeRecord 24 bytes** layout (1+1+2+4+4+4+4+4=24) ✓
- **Header 40 bytes** layout (sums of each offset are consistent) ✓
- **Kind number space**: single-instruction optimization and the contiguous JsdocBlock=0x01...JsdocText=0x0F assignment ✓
- **NodeList 0x7F, Sentinel 0x00** references match in every location ✓
- **Pos/End in UTF-16 code units, relative values, base_offset added on the JS side** convention is consistent ✓
- **compat_mode = Header bit0** + RemoteSourceFile retention + lazy decoder branching ✓
- **Phase 1.0a-d / 1.1a-d / 1.2a-d / 1.3** sub-phase structure aligns across implementation phases,
  tests, and bench schedule (excluding #6) ✓
- **JsdocTag's 8-bit Children bitmask** placement (Extended Data byte 0) and visitor-key bit ordering ✓
- **Section concatenation order** (Header → RootIndex → StringOffsets → StringData →
  ExtData → Diagnostics → Nodes) matches at L548 and L1827 ✓

---

## Recommended fix order

```
Phase A: Critical fixes (clear implementation blockers)
  #1 (JsdocBlock bitmask) → #2 (TypeMethodSignature bit) → #3 (TypeMethodSignature bitmask)

Phase B: Should Fix
  #4 (line numbers) → #5 (JsdocGenericTagBody) → #6 (Phase test schedule)

Phase C: Nice to Have
  #7 (quote style unification) → #8 (matrix table)
```

#1, #2, and #3 are **mutually independent** and can be done in parallel.
#4 and #5 are also independent. #6 is independent of the others.
#7 and #8 are independent of everything else.

---

## Additional fixes: consistency of dependent files under `.notes/` (2026-04-19)

All 9 references from the design document (`design/binary-ast-draft.md`) to files under
`.notes/` were verified, and **2 outdated descriptions were found and updated**.

### `binary-ast-batch-processing.md` (4 references)

**Problem**: The conclusion of Discussion 2 "Rust encoder API" still listed **approach c-2**
(parser → typed AST → binary writer, two-stage) whereas the current design uses
**approach c-1** (parser writes Binary AST directly into the arena, typed AST removed),
making the documents inconsistent.

**Fix**:

- L264-285: updated the conclusion's chosen option from c-2 to c-1 and added a change-history note.
- L283 (Rust walker): "walks the typed AST directly" → "walks the Binary AST via the lazy decoder".
- L552 (recommendation summary table): approach c-2 → c-1.

### `js-rust-transfer.md` (1 reference)

**Problem**: The entire "Approach" section was outdated, ending with the conclusion
"Phase 1 = JSON, Phase 2 = raw transfer". The current design has already adopted Binary AST.

**Fix**:

- Rewrote the entirety of L188-227, organizing the evolution of the transfer mechanism into three stages:
  1. Initial: JSON-based transfer (implemented).
  2. Interim consideration: oxc Raw Transfer → not adopted (no WASM support).
  3. **Current: tsgo-style Binary AST (approach c-1) adopted**.
- Documented the reasons the rejected options (Raw Transfer / JSON) were not adopted.
- Added a summary of lessons learned.

### Update on the design document side

Updated the reference description at L70 of `design/binary-ast-draft.md`:

- Old: `js-rust-transfer.md — selection rationale from the ox-jsdoc perspective`
- New: `js-rust-transfer.md — history of selecting JSON / Raw Transfer / Binary AST
(background leading to the adoption of Binary AST)`

### References confirmed consistent (no change required)

- `benchmark-results.md` — measurements from 2026-04-10, current.
- `tsgo/tsgo-binary-ast.md` — factual description of the tsgo specification, current.
- `tsgo-vs-oxc-ast-transfer.md` — tsgo vs. oxc comparison, current.
