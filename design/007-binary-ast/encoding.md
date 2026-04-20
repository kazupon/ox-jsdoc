# Encoding (Tree, Variant, compat_mode)

## Design overview

This document organizes the **conventions used during Binary AST encoding**
(how to write things out), split across three topics (expansion of Rust enum
variants / relationship with compat_mode / tree encoding). In contrast to
format.md (the format specification = what bytes look like), this document
addresses the **encoder's perspective on design decisions**.

Key decisions:

- **Expand Rust enum variants into independent Kinds**: expanding `JsdocTagBody`
  (3 variants) / `JsdocTagValue` (4 variants) into separate Kinds simplifies
  decoder dispatch and maintains ESTree compatibility (variant names visible
  via `node.type`)
- **`JsdocType` (Parsed/Raw) does not get a wrapper**: `JsdocTag.parsedType`
  directly points to a TypeNode (1:1 correspondence with the existing JSON
  output)
- **compat_mode is handled within the single format (Header bit0)**: no
  separate format or section; switches between basic / compat via a Header
  flag. Binary compatibility is preserved
- **Empty NodeLists are not emitted (Option A2)**: bit=0 in the parent's
  Children bitmask is treated semantically as "empty array". A reduction of
  about 1100 nodes / about 26 KB per 100-batch
- **The lazy decoder holds the compat flag at the root (`RemoteSourceFile`)**:
  child nodes traverse `parent → root → compat` to look it up; no flag is held
  per node (memory efficient)
- **Tree follows the same convention as tsgo**: reconstructed with `node[0]`
  sentinel + `parent` index + `next_sibling` link, with the
  `first_child = node[i+1]` rule

## Handling nodes with variants

The Rust enums (`JsdocType`, `JsdocTagBody`, `JsdocTagValue`) have few
variants, but each variant has a very different field structure. Policy:
**make each variant its own Kind**:

| Rust enum                        | Binary AST Kind                   |
| -------------------------------- | --------------------------------- |
| `JsdocTagBody::Generic(...)`     | `JsdocGenericTagBody` (Kind 0x09) |
| `JsdocTagBody::Borrows(...)`     | `JsdocBorrowsTagBody` (Kind 0x0A) |
| `JsdocTagBody::Raw(...)`         | `JsdocRawTagBody` (Kind 0x0B)     |
| `JsdocTagValue::Parameter(...)`  | `JsdocParameterName` (Kind 0x0C)  |
| `JsdocTagValue::Namepath(...)`   | `JsdocNamepathSource` (Kind 0x0D) |
| `JsdocTagValue::Identifier(...)` | `JsdocIdentifier` (Kind 0x0E)     |
| `JsdocTagValue::Raw(...)`        | `JsdocText` (Kind 0x0F)           |

Reasons:

- If we packed a variant tag into 6-bit Common Data, the decoder would have to
  dynamically determine each variant's field structure, complicating the
  generated code
- Splitting Kinds enables the same simple dispatch table as tsgo
- ESLint visitors can see the variant name directly via `node.type`,
  improving ESTree compatibility

Exception: `JsdocType` (Parsed|Raw) does not get a wrapper Kind:

- The existing JSON serializer emits `parsedType: TypeNode` (without a
  wrapper); the Binary AST is aligned to the same structure
- For `JsdocType::Parsed`: `JsdocTag.parsedType` directly points to a TypeNode
  (one of Kind 0x80-0xFF)
- For `JsdocType::Raw`: the parsedType field itself is omitted (rawType is
  held separately as a string index in Extended Data)

---

## Relationship with compat_mode

The current serializer has `SerializeOptions { compat_mode,
empty_string_for_null, include_positions }`. The Binary AST handles this
within the single format (Option A: switching via a Header flag):

### Adoption policy

1. **Indicate compat_mode with a Header flag** (bit0)
   - Without creating a separate format (Option B) or separate section (Option D),
     **support both modes within one binary format**
   - Binary compatibility is preserved (a non-compat decoder can still read the
     Header on a compat buffer)
   - Avoids doubling the Kind count
2. **When compat_mode is ON during encoding, also write the additional metadata**:
   - `JsdocBlock`: 6 line indices + delimiter string indices
   - `JsdocTag`: x7 delimiter string indices
   - `JsdocDescriptionLine` / `JsdocTypeLine`: delimiter string indices
3. **The decoder reads or skips the compat region based on the Header flag**
4. **`empty_string_for_null` is applied on the decoder side**: in the Binary
   AST it is always represented as `Option` (the sentinel for None depends on
   the storage location: **30-bit slot in Node Data is `0x3FFFFFFF`**, **u16
   slot in Extended Data is `0xFFFF`**)
   - Most fields live in Extended Data (string fields of JsdocBlock, JsdocTag,
     JsdocInlineTag, etc., are all u16)
   - The 30-bit string index in Node Data is only used by String-type nodes
     (TypeName, TypeNumber, etc.)
5. **`include_positions` is always true** (Pos/End are fixed fields)
6. **The lazy decoder holds the compat flag at the `RemoteSourceFile` (root)**:
   child nodes traverse `parent → root → compat` to look it up
   (no flag per node, prioritizing memory efficiency)

### Size impact

Non-compat: ~24 bytes/node average
compat: ~32-40 bytes/node average (due to increased Extended Data size)

### Lazy decoder implementation sketch

```javascript
class RemoteSourceFile {
  #compatMode // Retrieved and held from Header bit0

  get compatMode() {
    return this.#compatMode
  }
}

class RemoteJsdocBlock {
  // Basic part (always present, offset 0-17)
  get childrenBitmask() {
    return this.view.getUint8(this.extendedDataOffset + 0) // bit0=descLines, bit1=tags, bit2=inlineTags
  }
  // byte 1 is alignment padding
  get description() {
    const idx = this.view.getUint16(this.extendedDataOffset + 2, true)
    return idx === 0xffff ? null : this.sourceFile.getString(idx)
  }
  get delimiter() {
    const idx = this.view.getUint16(this.extendedDataOffset + 4, true)
    return this.sourceFile.getString(idx)
  }
  // post_delimiter at +6, terminal +8, line_end +10, initial +12,
  // delimiter_line_break +14, preterminal_line_break +16

  // compat extension part (only present when compat_mode; actual data at offset 20-39, 18-19 is alignment)
  // The compat flag is referenced through sourceFile
  get endLine() {
    if (!this.sourceFile.compatMode) return undefined
    return this.view.getUint32(this.extendedDataOffset + 20, true)
  }
  get descriptionStartLine() {
    if (!this.sourceFile.compatMode) return undefined
    const v = this.view.getUint32(this.extendedDataOffset + 24, true)
    return v === 0xffffffff ? undefined : v // Sentinel represents None
  }
  // descriptionEndLine: +28, lastDescriptionLine: +32
  // hasPreterminalDescription: +36 (u8)
  // hasPreterminalTagDescription: +37 (u8, 0xFF = None)
  // ...
}
```

The Rust-side `LazyJsdocBlock` likewise references the root via
`LazySourceFile.compat_mode()`.

---

## Tree encoding

Nodes are stored as a flat array in source order. They are reconstructed using
the same rules as tsgo:

1. `node[0]` is the sentinel (all fields zero)
2. `node[1]` is the root (`JsdocBlock`)
3. The first child of `node[i]` is `node[i+1]` (when `node[i+1].parent == i`)
4. Siblings are linked via the `next_sibling` field
5. When there is no child or no sibling, the value is 0 (= points to the sentinel)
6. Following the tsgo `NodeList` (Kind 0x7F in ox-jsdoc) pattern, array fields
   are wrapped in a single node (`tags[]`, `description_lines[]`, etc.)

### NodeList optimization: skip empty arrays (Option A2)

Because NodeList overhead accumulates during batch processing (empty arrays
in JSDoc comments can account for more than half of the node count), we adopt
the rule of **not emitting NodeLists for empty arrays**:

```text
Parent node's Children bitmask:
  bit n = 1 → emit NodeList (length=0 is allowed but rare)
  bit n = 0 → do not emit NodeList (= treated as empty array)
```

**Semantic unification**:

- In the ox-jsdoc Rust AST, `tags: Vec<JsdocTag>` (not Option, can be empty)
- In the Binary AST, "**empty array**" and "**field absent**" are **treated
  identically** (represented by bit 0)
- From the ESLint plugin's perspective, the behavior with `tags.length === 0`
  is the same

**JS decoder implementation**:

```javascript
class RemoteJsdocBlock {
  get tags() {
    if (!(this.childMask & TAGS_BIT)) return EMPTY_NODE_LIST // bit 0 → shared empty NodeList
    return this.#getNodeList(TAGS_NODE_INDEX) // bit 1 → RemoteNodeList instance
  }
}
```

The return type is `RemoteNodeList extends Array` (see [js-decoder.md](./js-decoder.md)
"Return type for array fields" for details).
Empty arrays are represented by a shared singleton (`EMPTY_NODE_LIST`,
`length === 0`).

**Effect**: a 100-comment batch removes about 1100 empty-array NodeLists
(~600 `inlineTags` + ~500 `typeLines`, etc.) → **about 26 KB reduction**.

**Exception**: cases where the NodeList must always be emitted (noted as
future extension room):

- Cases where the array length is "0 but not empty" (no such case in the
  current ox-jsdoc AST)
- When the existence of the empty NodeList itself carries meaning
- This exception is not currently needed (extend the spec if it becomes
  necessary)
