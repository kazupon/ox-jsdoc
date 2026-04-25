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
- **Variable-length child lists live as direct children of the parent**:
  each list owns a 6-byte `(head_index: u32, count: u16)` metadata slot
  inside the parent's Extended Data block (see
  [format.md](./format.md#list-metadata-in-extended-data)). Decoders read
  `(head, count)` and walk `next_sibling` exactly `count` times
- **Empty lists are encoded as `(head=0, count=0)`**: the parent's Children
  bitmask bit for that list is also cleared so visitor frameworks can
  shortcut empty traversals before reading the metadata slot
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

| Rust enum                        | Binary AST Kind                                  |
| -------------------------------- | ------------------------------------------------ |
| `JsdocTagBody::Generic(...)`     | `JsdocGenericTagBody` (Kind 0x09)                |
| `JsdocTagBody::Borrows(...)`     | `JsdocBorrowsTagBody` (Kind 0x0A) — **reserved** |
| `JsdocTagBody::Raw(...)`         | `JsdocRawTagBody` (Kind 0x0B) — **reserved**     |
| `JsdocTagValue::Parameter(...)`  | `JsdocParameterName` (Kind 0x0C)                 |
| `JsdocTagValue::Namepath(...)`   | `JsdocNamepathSource` (Kind 0x0D)                |
| `JsdocTagValue::Identifier(...)` | `JsdocIdentifier` (Kind 0x0E)                    |
| `JsdocTagValue::Raw(...)`        | `JsdocText` (Kind 0x0F)                          |

> **Reserved Kinds**: `JsdocBorrowsTagBody` (0x0A) / `JsdocRawTagBody` (0x0B)
> are reserved discriminants — neither the typed parser nor the binary
> parser currently produces `JsdocTagBody::Borrows` or `JsdocTagBody::Raw`.
> `@borrows source as target` and similar bodies are emitted as
> `JsdocGenericTagBody` (0x09). The Kinds, the writer helpers
> (`write_jsdoc_borrows_tag_body` / `write_jsdoc_raw_tag_body`) and the
> decoder classes are kept as scaffolding for a future specialization.

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
  held separately as a child `JsdocTypeSource` string-leaf node)

---

## Relationship with compat_mode

The compat-mode toggle lives on the parser side as
`ParseOptions::compat_mode` (binary AST writer / decoder) and on the
typed-AST serializer side as `SerializeOptions::compat_mode` plus its
companion fields `empty_string_for_null`, `include_positions`, `spacing`,
and `position_map`. The Binary AST handles compat-mode within the single
format (Option A: switching via a Header flag):

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
   AST it is always represented as `Option`. The sentinel for None depends on
   the storage location:
   - **`StringField` slot in Extended Data** (most string fields live here —
     JsdocBlock, JsdocTag, JsdocInlineTag, …): `(offset = 0xFFFF_FFFF, length = 0)`
   - **30-bit Node Data payload, `TypeTag::String` (long-string fallback for
     string-leaf nodes)**: `0x3FFF_FFFF`
   - **30-bit Node Data payload, `TypeTag::StringInline` (short-string fast
     path for string-leaf nodes)**: never None — the encoder only emits
     `StringInline` for present, in-range values; absent strings stay on
     `TypeTag::String` (`0x3FFF_FFFF` sentinel) or on the `Extended` path
     (`StringField::NONE`)
   - String-leaf node Kinds (TypeName, TypeNumber, JsdocText, JsdocTagName,
     …) use either `StringInline` or `String` — encoder picks per-emit by
     length / offset; see `format.md#stringinline-0b11` for the rule
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
  // Basic part (always present, offset 0-67 in basic mode; 0-89 in compat mode)
  get childrenBitmask() {
    return this.view.getUint8(this.extendedDataOffset + 0) // bit0=descLines, bit1=tags, bit2=inlineTags
  }
  // byte 1 is alignment padding

  // String fields: 8 inline `StringField` slots (6 bytes each = u32 offset + u16 length)
  // at bytes 2-49 — readers slice String Data directly via getStringByField.
  get description() {
    const off = this.extendedDataOffset + 2
    const offset = this.view.getUint32(off, true)
    const length = this.view.getUint16(off + 4, true)
    return offset === 0xffff_ffff ? null : this.sourceFile.getStringByField(offset, length)
  }
  get delimiter() {
    const off = this.extendedDataOffset + 8
    const offset = this.view.getUint32(off, true)
    const length = this.view.getUint16(off + 4, true)
    return this.sourceFile.getStringByField(offset, length) ?? ''
  }
  // post_delimiter at +14, terminal +20, line_end +26, initial +32,
  // delimiter_line_break +38, preterminal_line_break +44 (each StringField is 6 bytes)

  // List metadata: 3 × 6-byte (head_index: u32, count: u16) slots at bytes 50-67.
  // descriptionLines metadata at +50, tags at +56, inlineTags at +62.
  get tags() {
    const off = this.extendedDataOffset + 56 // JSDOC_BLOCK_TAGS_SLOT
    const head = this.view.getUint32(off, true)
    const count = this.view.getUint16(off + 4, true)
    return head === 0 || count === 0
      ? EMPTY_NODE_LIST
      : nodeListFromMetadata(this.sourceFile, head, count, /* parent */ this)
  }
  // descriptionLines and inlineTags follow the same pattern at +50 and +62.

  // Compat extension part (only present when compat_mode; basic ends at byte 68;
  // bytes 68-69 are alignment padding for the compat tail)
  // The compat flag is referenced through sourceFile.
  get endLine() {
    if (!this.sourceFile.compatMode) return undefined
    return this.view.getUint32(this.extendedDataOffset + 70, true)
  }
  get descriptionStartLine() {
    if (!this.sourceFile.compatMode) return undefined
    const v = this.view.getUint32(this.extendedDataOffset + 74, true)
    return v === 0xffff_ffff ? undefined : v // sentinel represents None
  }
  // descriptionEndLine: +78, lastDescriptionLine: +82
  // hasPreterminalDescription: +86 (u8)
  // hasPreterminalTagDescription: +87 (u8, 0xFF = None)
  // bytes 88-89 are trailing alignment padding (compat ED ends at byte 90)
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
6. Variable-length child lists (`tags[]`, `description_lines[]`,
   `interpolations[]`, etc.) are stored as direct children of the parent. Each
   list has a 6-byte `(head_index: u32, count: u16)` slot in the parent's
   Extended Data block — see [List metadata in Extended Data](./format.md#list-metadata-in-extended-data)

### List metadata: empty-list encoding

Each variable-length child list has its metadata stored inline in the
parent's Extended Data block:

```text
Per list (6 bytes):
  byte 0-3: head_index (u32) — node index of the list's first element
  byte 4-5: count      (u16) — number of elements (0 for empty)
```

Empty lists are encoded as `(head=0, count=0)`; the parent's Children bitmask
bit for that list is also cleared so a fast `(bitmask & X) != 0` check can
shortcut the metadata read.

**Semantic unification**:

- In the ox-jsdoc Rust AST, `tags: Vec<JsdocTag>` (not Option, can be empty)
- In the Binary AST, an absent or empty list is encoded the same way
  (`count = 0`); decoders return an immediately-empty iterator
- From the ESLint plugin's perspective, the behavior with `tags.length === 0`
  is the same

**JS decoder implementation**:

```javascript
class RemoteJsdocBlock {
  get tags() {
    const bitmask = this.#readExtByte(0)
    if (!(bitmask & TAGS_BIT)) return EMPTY_NODE_LIST
    const head = this.#readExtU32(JSDOC_BLOCK_TAGS_SLOT)
    const count = this.#readExtU16(JSDOC_BLOCK_TAGS_SLOT + 4)
    return new RemoteNodeList(this.bytes, head, count, this.rootIndex)
  }
}
```

`RemoteNodeList extends Array` (see [js-decoder.md](./js-decoder.md)
"Return type for array fields" for details). Empty arrays are represented by
a shared singleton (`EMPTY_NODE_LIST`, `length === 0`).
