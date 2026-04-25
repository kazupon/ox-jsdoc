# JS Decoder (JS lazy decoder)

## Design overview

The JS-side Binary AST decoder is designed to deliver **lazy expansion + zero-copy + full environment support
(Node/Deno/Bun/Browser)**. It mirrors the Rust side (LazySourceFile + LazyJsdocBlock, etc.) with a
**fully symmetric structure**, enabling both languages to be generated simultaneously from a single AST schema.

Key decisions:

- **Lazy expansion (proxies built only on access)**: rather than materializing every node up front, `getter`s
  read the DataView only when needed. Unvisited nodes cost zero
- **Single `#internal` object pattern**: following oxc raw transfer, private state and cache slots
  (`$tag`, `$rawType`, etc.) are consolidated into a single object. The V8 hidden class stays stable and
  declarations remain concise
- **Shared decoder package `@ox-jsdoc/decoder`**: the NAPI binding and WASM binding share **fully identical
  implementations**. Because it does not depend on the byte source (NAPI Buffer / wasm.memory.buffer view),
  format changes propagate to both bindings simultaneously
- **toJSON + Symbol.for("nodejs.util.inspect.custom")**: ECMA-standard `JSON.stringify` performs eager
  conversion, and `console.log` prints directly in Node-family runtimes. It also works in browsers via
  `JSON.stringify` (a `toPlainObject` helper is also provided)
- **`RemoteNodeList extends Array`** + **`EMPTY_NODE_LIST` shared singleton**: array fields expose an
  `Array`-compatible API; empty arrays use a singleton for memory efficiency
- **Kind dispatch is code-generated in Phase 4**: a flat 256-entry `KIND_TABLE` for O(1) lookup. Single-instruction
  category checks leverage the Kind numbering constraints (Sentinel 0x00 / NodeList 0x7F (reserved-only,
  never emitted) / TypeNode 0x80-0xFF) ([ast-nodes.md "Implementation of category checks"](./ast-nodes.md#category-check-implementation))

This document explains the **Lazy node classes** (`#internal` pattern + main examples + helpers + RemoteSourceFile +
Kind dispatch) → **Eager conversion** → **Array fields** → **Visitor Keys** → **JS Public API**, in that order.

## Lazy node classes

We follow tsgo's `RemoteNode` pattern, additionally adopting oxc raw transfer (lazy mode)'s `toJSON()` +
Node.js inspect support. Each node class is merely a **view + accessor set** over the binary buffer; values
are computed only on property access.

### Internal state encapsulation pattern (`#internal`)

Following oxc raw transfer (lazy mode), private state is consolidated into a **single `#internal` object**:

- Private state declarations fit on one line, and the constructor stays simple
- `$tag`, `$rawType`, etc. — **caches for lazily constructed properties** — live in the same place,
  preventing recomputation on re-access
- The V8 hidden class remains stable (instances share a single shape → faster property lookup)

Contents of `#internal` (common to all node classes):

| Key          | Type               | Usage                                                                                      |
| ------------ | ------------------ | ------------------------------------------------------------------------------------------ |
| `view`       | DataView           | View over the entire binary buffer                                                         |
| `byteIndex`  | u32                | Byte offset of this node record (= `nodesOffset + index * 24`)                             |
| `index`      | u32                | Index of the node record (= `(byteIndex - nodesOffset) / 24`)                              |
| `rootIndex`  | u32                | Index of the root this node belongs to (used to obtain `baseOffset` for range computation) |
| `parent`     | RemoteNode \| null | Reference to the parent node (also provides `compat_mode` via sourceFile)                  |
| `sourceFile` | RemoteSourceFile   | The root that consolidates the string table / Header / Root array                          |
| `$xxx`       | any                | Cache for child nodes / arrays / computed values (lazily built)                            |

### Main node class example: RemoteJsdocTag

```javascript
const inspectSymbol = Symbol.for('nodejs.util.inspect.custom')

// JsdocTag Extended Data layout (basic 38 bytes, see format.md):
//   byte 0: Children bitmask (u8) — retained for the visitor framework
//     bit0=tag, bit1=rawType, bit2=name, bit3=parsedType, bit4=body,
//     bit5=typeLines, bit6=descLines, bit7=inlineTags
//   byte 1: padding
//   byte 2-7  : default_value StringField (offset u32 + length u16)
//                                          NONE = (offset=0xFFFF_FFFF, length=0)
//   byte 8-13 : description   StringField  (NONE if absent)
//   byte 14-19: raw_body      StringField  (NONE if absent)
//   byte 20-25: typeLines       list metadata (head_index: u32, count: u16)
//   byte 26-31: descriptionLines list metadata (ditto)
//   byte 32-37: inlineTags      list metadata (ditto)
// tag/rawType/name/parsedType/body are span-bearing structs and are placed as **child nodes**
// → check the corresponding bit in the Children bitmask; if bit=1, fetch the child at the matching visitor index
// The 3 list slots (typeLines / descriptionLines / inlineTags) live as direct
// siblings under the parent and are walked via `(head_index, count)` per-list metadata.

export class RemoteJsdocTag {
  type = 'JsdocTag'
  #internal // { view, byteIndex, index, rootIndex, parent, sourceFile, $tag, ... }

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this.#internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }

  get tag() {
    if (this.#internal.$tag !== undefined) return this.#internal.$tag
    // bit0=tag (required, so bitmask is always 1) → child at visitor index 0
    return (this.#internal.$tag = this.#childAt(0)) // RemoteJsdocTagName
  }

  get rawType() {
    if (this.#internal.$rawType !== undefined) return this.#internal.$rawType
    if ((this.#bitmask() & 0b0000_0010) === 0) return (this.#internal.$rawType = null)
    return (this.#internal.$rawType = this.#childAt(1)) // RemoteJsdocTypeSource
  }

  get parsedType() {
    if (this.#internal.$parsedType !== undefined) return this.#internal.$parsedType
    if ((this.#bitmask() & 0b0000_1000) === 0) return (this.#internal.$parsedType = null)
    return (this.#internal.$parsedType = this.#childAt(3)) // TypeNode (Kind 0x80-0xFF)
  }

  // Pure string fields stored as inline 6-byte StringField slots in Extended Data
  get defaultValue() {
    return this.#stringFieldAt(2)
  }
  get description() {
    return this.#stringFieldAt(8)
  }
  get rawBody() {
    return this.#stringFieldAt(14)
  }

  get range() {
    const { view, byteIndex, rootIndex, sourceFile } = this.#internal
    const pos = view.getUint32(byteIndex + 4, true)
    const end = view.getUint32(byteIndex + 8, true)
    const baseOffset = sourceFile.getRootBaseOffset(rootIndex)
    return [baseOffset + pos, baseOffset + end]
  }

  get parent() {
    return this.#internal.parent
  }

  // --- Internal helpers (shared logic for reading Extended Data) ---
  #extOffset() {
    return extOffsetOf(this.#internal)
  } // see below
  #bitmask() {
    return this.#internal.view.getUint8(this.#extOffset())
  }
  #childAt(visitorIndex) {
    return childAtVisitorIndex(this.#internal, visitorIndex)
  }
  #stringFieldAt(o) {
    // Read 6-byte StringField inline: u32 offset at +0, u16 length at +4.
    const ext = this.#extOffset() + o
    const offset = this.#internal.view.getUint32(ext, true)
    const length = this.#internal.view.getUint16(ext + 4, true)
    return this.#internal.sourceFile.getStringByField(offset, length)
  }

  // ECMA standard: invoked by JSON.stringify
  toJSON() {
    return {
      type: 'JsdocTag',
      range: this.range,
      tag: this.tag.toJSON(), // Required (JsdocTagName)
      rawType: this.rawType?.toJSON() ?? null, // Option (JsdocTypeSource)
      parsedType: this.parsedType?.toJSON() ?? null, // Option (TypeNode)
      description: this.description,
      defaultValue: this.defaultValue,
      rawBody: this.rawBody
      // ... other fields (name, body, typeLines, descLines, inlineTags, etc., the same)
    }
  }

  // Node.js / Deno / Bun console.log support (ignored in browsers)
  [inspectSymbol]() {
    return Object.setPrototypeOf(this.toJSON(), DebugJsdocTag.prototype)
  }
}

// Empty class for the inspect-time class label (oxc-style trick)
const DebugJsdocTag = class JsdocTag {}
```

Properties are computed on access, and after the first computation are cached in `#internal.$xxx`.

### Helper functions (shared parts for reading the Binary AST)

These are implemented as helpers in the decoder package and called from each node class:

```javascript
// 1) Node Data → byte offset within Extended Data (Extended type 0b10 only)
//    Node Data layout: [31:30]=type tag (2 bits) + [29:0]=payload (30 bits)
export function extOffsetOf(internal) {
  const { view, byteIndex, sourceFile } = internal
  const nodeData = view.getUint32(byteIndex + 12, true) // byte 12-15
  const typeTag = (nodeData >>> 30) & 0b11
  if (typeTag !== 0b10) {
    throw new Error(`Node at ${byteIndex} is not Extended type (got 0b${typeTag.toString(2)})`)
  }
  return sourceFile.extendedDataOffset + (nodeData & 0x3fff_ffff)
}

// 2) Children bitmask + visitor index → instance of the corresponding child node
//    Child nodes are placed contiguously in DFS pre-order starting right after the parent (parent_index + 1)
//    Visitor index n = the n-th set bit on the bitmask
export function childAtVisitorIndex(internal, visitorIndex) {
  const { view, index, sourceFile } = internal
  const bitmask = view.getUint8(extOffsetOf(internal)) // Extended Data byte 0

  // Walk the set bits in visitor order and return the child at the target visitor index
  let childIdx = index + 1
  let visitorPos = 0
  for (let bit = 0; bit < 8; bit++) {
    if (!(bitmask & (1 << bit))) continue // This slot was not emitted
    if (visitorPos === visitorIndex) {
      return sourceFile.getNode(childIdx, /* parent */ thisNode(internal))
    }
    // Advance to the next child (= next_sibling of the current child)
    childIdx = view.getUint32(sourceFile.nodesOffset + childIdx * 24 + 20, true)
    if (childIdx === 0) return null // No-children sentinel
    visitorPos++
  }
  return null
}

// 3) Read per-list `(head_index, count)` metadata from the parent's Extended Data
//    block at `slotOffset` and build a RemoteNodeList. List children are direct
//    siblings under the parent (no NodeList wrapper); the iterator walks
//    `next_sibling` exactly `count` times. Empty lists share EMPTY_NODE_LIST.
export function nodeListAtSlotExtended(internal, slotOffset) {
  const ext = extOffsetOf(internal) + slotOffset
  const head = internal.view.getUint32(ext, true)
  const count = internal.view.getUint16(ext + 4, true)
  if (head === 0 || count === 0) return EMPTY_NODE_LIST
  return collectNodeListChildren(internal, head, count)
}

// 4) Resolve the 30-bit Node Data string-leaf payload into a string.
//    Dispatches on the 2-bit TypeTag:
//    - 0b01 (`String`)       → 30-bit String Offsets index, fallback for long strings
//    - 0b11 (`StringInline`) → packed `(offset:u22, length:u8)` directly into String Data
export function stringPayloadOf(internal) {
  const { view, byteIndex, sourceFile } = internal
  const nodeData = view.getUint32(byteIndex + 12, true)
  const tag = (nodeData >>> 30) & 0b11
  const payload = nodeData & 0x3fff_ffff
  if (tag === 0b11) {
    // StringInline: low 8 bits = length, upper 22 bits = offset
    const length = payload & 0xff
    const offset = payload >>> 8
    return sourceFile.getStringByOffsetAndLength(offset, length)
  }
  // String (0b01): None sentinel = 0x3FFF_FFFF
  if (payload === 0x3fff_ffff) return null
  return sourceFile.getString(payload)
}
```

### RemoteSourceFile (the decoder's root class)

The **decoder entry point** that consolidates the Header / Root array / String table.
Every node class goes through this to obtain strings, base offsets, nodeCount, etc.

```javascript
import { decodeKindToClass } from './kind-dispatch.js'

const utf8Decoder = new TextDecoder('utf-8')

export class RemoteSourceFile {
  #internal

  constructor(buffer) {
    const view = new DataView(buffer)
    // Read the 40-byte Header in one pass (offsets / counts are cached)
    this.#internal = {
      view,
      version: view.getUint8(0),
      compatMode: (view.getUint8(1) & 0x01) !== 0,
      rootArrayOffset: view.getUint32(4, true),
      stringOffsetsOffset: view.getUint32(8, true),
      stringDataOffset: view.getUint32(12, true),
      extendedDataOffset: view.getUint32(16, true),
      diagnosticsOffset: view.getUint32(20, true),
      nodesOffset: view.getUint32(24, true),
      nodeCount: view.getUint32(28, true),
      sourceTextLength: view.getUint32(32, true),
      rootCount: view.getUint32(36, true),
      stringCache: new Map(),
      nodeCache: new Array(view.getUint32(28, true))
    }
  }

  // Public getters (referenced by each node class)
  get compatMode() {
    return this.#internal.compatMode
  }
  get extendedDataOffset() {
    return this.#internal.extendedDataOffset
  }
  get nodesOffset() {
    return this.#internal.nodesOffset
  }
  get rootCount() {
    return this.#internal.rootCount
  }

  // String Offsets[idx] → String Data slice (UTF-8 → UTF-16 via TextDecoder).
  // Used by string-leaf nodes whose Node Data carries a `TypeTag::String`
  // payload (long-string fallback path) and the diagnostics section's
  // `message_index`.
  getString(idx) {
    if (idx === 0x3fff_ffff) return null
    const cached = this.#internal.stringCache.get(idx)
    if (cached !== undefined) return cached
    const { view, stringOffsetsOffset, stringDataOffset } = this.#internal
    const start = view.getUint32(stringOffsetsOffset + idx * 8, true)
    const end = view.getUint32(stringOffsetsOffset + idx * 8 + 4, true)
    const bytes = new Uint8Array(view.buffer, stringDataOffset + start, end - start)
    const str = utf8Decoder.decode(bytes)
    this.#internal.stringCache.set(idx, str)
    return str
  }

  // Inline `StringField` `(offset, length)` → String Data slice. Used by
  // Extended Data string slots which embed `(offset, length)` directly
  // without going through the offsets table.
  // None sentinel: `offset === 0xFFFF_FFFF` (length is 0).
  getStringByField(offset, length) {
    if (offset === 0xffff_ffff) return null
    return this.getStringByOffsetAndLength(offset, length)
  }

  // Resolve a Path B-leaf inline `(offset, length)` pair into the underlying
  // string. Used by both `TypeTag::StringInline` (Node Data) and
  // `StringField` (Extended Data) when the slot is non-NONE. Always returns
  // a real string — neither encoding emits this for absent values.
  getStringByOffsetAndLength(offset, length) {
    // Cache key: high-bit-set form so it never collides with index-keyed
    // entries from getString().
    const cacheKey = -(offset + 1)
    const cached = this.#internal.stringCache.get(cacheKey)
    if (cached !== undefined) return cached
    const { view, stringDataOffset } = this.#internal
    const bytes = new Uint8Array(view.buffer, stringDataOffset + offset, length)
    const str = utf8Decoder.decode(bytes)
    this.#internal.stringCache.set(cacheKey, str)
    return str
  }

  // base_offset of the i-th entry in the root array (used for absolute offset calculations)
  getRootBaseOffset(rootIndex) {
    const { view, rootArrayOffset } = this.#internal
    return view.getUint32(rootArrayOffset + rootIndex * 12 + 8, true)
  }

  // Any node index → corresponding RemoteXxx instance (lazy + cached)
  getNode(nodeIndex, parent, rootIndex = -1) {
    if (nodeIndex === 0) return null // sentinel
    const cached = this.#internal.nodeCache[nodeIndex]
    if (cached !== undefined) return cached
    const byteIndex = this.#internal.nodesOffset + nodeIndex * 24
    const kind = this.#internal.view.getUint8(byteIndex)
    const Class = decodeKindToClass(kind)
    const node = new Class(this.#internal.view, byteIndex, nodeIndex, rootIndex, parent, this)
    this.#internal.nodeCache[nodeIndex] = node
    return node
  }

  // The AST of each root (passed to ParseResult.asts)
  get asts() {
    if (this.#internal.$asts !== undefined) return this.#internal.$asts
    const { view, rootArrayOffset, rootCount } = this.#internal
    const result = new Array(rootCount)
    for (let i = 0; i < rootCount; i++) {
      const nodeIdx = view.getUint32(rootArrayOffset + i * 12, true)
      result[i] = nodeIdx === 0 ? null : this.getNode(nodeIdx, null, i)
    }
    return (this.#internal.$asts = result)
  }
}
```

### Kind dispatch (Kind → class selection)

Look up the corresponding `RemoteXxx` class from the **Kind value (u8)** at byte 0 of the node record.
This is **code-generated** in Phase 4 (see ast-nodes.md), enabling O(1) lookup via a flat array:

```javascript
// generated/kind-dispatch.js (auto-generated in Phase 4)
import {
  RemoteJsdocBlock,
  RemoteJsdocDescriptionLine,
  RemoteJsdocTag,
  RemoteJsdocTagName,
  RemoteJsdocTagNameValue,
  RemoteJsdocTypeSource
  // ... 15 comment AST kinds
} from './nodes/jsdoc.js'
import {
  RemoteTypeName,
  RemoteTypeNumber,
  RemoteTypeUnion
  // ... 45 TypeNode kinds
} from './nodes/type-nodes.js'
import { RemoteNodeListNode } from './nodes/node-list.js'

// Flat 256-entry table (single-instruction lookup, undefined for missing entries)
const KIND_TABLE = new Array(256)
KIND_TABLE[0x00] = null // Sentinel — getNode(0) short-circuits, so it never reaches here
KIND_TABLE[0x01] = RemoteJsdocBlock
KIND_TABLE[0x02] = RemoteJsdocDescriptionLine
KIND_TABLE[0x03] = RemoteJsdocTag
// ... 0x04-0x0F (remaining comment AST)
// NodeList (Kind 0x7F) is a reserved discriminant kept for legacy buffer
// compatibility but the encoder no longer emits it (lists are stored as
// inline `(head_index, count)` metadata in the parent's Extended Data).
// Mapped to a class purely as a defensive fallback for older buffers; the
// hot path never reaches this entry.
KIND_TABLE[0x7f] = RemoteNodeListNode
KIND_TABLE[0x80] = RemoteTypeName
KIND_TABLE[0x81] = RemoteTypeNumber
// ... 0x82-0xFF (remaining TypeNodes)

export function decodeKindToClass(kind) {
  const Class = KIND_TABLE[kind]
  if (Class === undefined) {
    throw new Error(`Unknown Kind: 0x${kind.toString(16).padStart(2, '0')}`)
  }
  return Class
}
```

For category checks (`is_type_node` / `is_node_list`, etc.), see [ast-nodes.md "Implementation of category checks"](./ast-nodes.md#category-check-implementation).
The Kind numbering space is designed to support single-instruction category checks under the constraints
**TypeNode = upper bit set**, **NodeList = 0x7F**, and **Sentinel = 0x00**.

## Eager conversion (toJSON / full environment support)

While only the lazy decoder is provided, implementing `toJSON()` on each node class enables eager conversion
via the ECMA-standard `JSON.stringify` (the same approach as oxc raw transfer):

```javascript
// Works in every environment (Node / Deno / Bun / Browser)
const result = parseBatch(items)
const block = result.asts[0]
const json = JSON.stringify(block, null, 2)
console.log(json) // The complete AST is output as a JSON string
```

### Debugging experience in the Node.js family (Node / Deno / Bun)

Implementing `Symbol.for('nodejs.util.inspect.custom')` makes `console.log(block)` display eagerly:

```text
RemoteJsdocBlock { type: 'JsdocBlock', range: [0, 50], tags: [ JsdocTag { ... } ] }
                                                              ^^^^^^^^
                                                              Empty class for label display
```

The trick `Object.setPrototypeOf(this.toJSON(), DebugXxx.prototype)` makes the output label `JsdocBlock`
instead of `Object` (improving debug readability).

In browsers `Symbol.for('nodejs.util.inspect.custom')` is ignored, so the above logic is harmless (no side effects).

### Browser / WASM environments

In the browser, `console.log(block)` alone shows only internal representation, so explicitly call `toJSON()`
or use a helper function:

```javascript
import { toPlainObject } from '@ox-jsdoc/decoder'

const result = parseBatch(items)

// Option 1: call toJSON() directly
console.log(block.toJSON())

// Option 2: via JSON.stringify
console.log(JSON.stringify(block, null, 2))

// Option 3: helper function
console.log(toPlainObject(block))
```

### Helper function implementation

Exported from the decoder package:

```javascript
// @ox-jsdoc/decoder package entry
export function toPlainObject(node) {
  if (node === null || node === undefined) return node
  if (typeof node !== 'object') return node
  if (Array.isArray(node)) return node.map(toPlainObject)
  if (typeof node.toJSON === 'function') return node.toJSON()
  return node // Already a plain object
}
```

The README guides browser users to **use `toPlainObject`**.

### Rejected alternatives

- **Full materialization via an `eager: true` option**: rejected because it loses the benefit of laziness
- **Proxy-based automatic eager conversion**: rejected due to V8 performance degradation
- **Browser DevTools-only formatter** (`window.devtoolsFormatters`): rejected because it is not standardized
  and requires users to change DevTools settings (oxc does not provide it either)
- **Custom `toString()` implementation**: rejected because the use case is too narrow (use `JSON.stringify(node)`
  if needed)

## Array field return type (RemoteNodeList)

Getters for array fields (`tags`, `descriptionLines`, `inlineTags`, etc.) return a **`RemoteNodeList`** instance.
This is a **subclass of `Array`** modeled on the same-named class in tsgo:

```javascript
// Provided by the shared decoder package
export class RemoteNodeList extends Array {
  // Inheriting from Array gives us length / map / filter / forEach and other standard methods
  // Each element (this[index]) is a lazy class instance (constructed on demand from the DataView)
  // Mutating methods (push/pop/splice, etc.) are not expected to be called (read-only)
}

// Empty arrays use a shared singleton (avoid `new` each time, prioritizing memory efficiency)
export const EMPTY_NODE_LIST = new RemoteNodeList() // length === 0
```

Callers can treat it as an ordinary `Array`:

```javascript
const result = parse('/** @param x @returns y */')
const tags = result.ast.tags // RemoteNodeList
console.log(tags.length) // 2
const names = tags.map(t => t.tag) // ['param', 'returns']
const params = tags.filter(t => t.tag === 'param') // [...]
for (const tag of tags) {
  /* ... */
} // for-of OK
```

**Design decision: adopt tsgo style (Array inheritance), reject oxc style (Proxy wrapper)**:

- tsgo style `extends Array`: simple, V8-friendly optimization, `Array` API compatible
- oxc style `Proxy`: flexible but high overhead (degrades performance of lazy index access)
- ox-jsdoc array fields are typically 1-10 elements, so Proxy flexibility is unnecessary

**Empty array handling**:

- "Empty array field" and "no field" are treated as semantically identical (NodeList skip optimization,
  consistent with Option A2)
- Both return `EMPTY_NODE_LIST` (a shared singleton) → no need to distinguish (just check `length === 0`)
- Callers use `tags.length === 0` to handle both cases

## Visitor Keys

Visitor keys for use by ESLint plugins are **planned** to be code-generated
from the AST schema. Manual maintenance cannot keep up with the 60 kinds,
so the export below is the **target shape**, not a currently-shipped
artifact: as of this writing `@ox-jsdoc/decoder` does **not** export
`jsdocVisitorKeys`. Consumers that need a visitor key map today should
inline the table below or wait for the codegen rollout (tracked alongside
the Phase 4 codegen work — see
[`README.md`](./README.md) "Code generation").

```javascript
// generated/visitor-keys.js (planned export — not yet shipped)
export const jsdocVisitorKeys = {
  JsdocBlock: ['descriptionLines', 'tags', 'inlineTags'],
  // parsedType points directly to a TypeNode (TypeName, etc.)
  // tag, rawType, name are span-bearing structs and become child nodes
  // (default_value, description, raw_body are pure &str and live in Extended Data)
  JsdocTag: [
    'tag',
    'rawType',
    'name',
    'parsedType',
    'body',
    'typeLines',
    'descriptionLines',
    'inlineTags'
  ],
  JsdocDescriptionLine: [],
  JsdocTypeLine: [],
  // `JsdocInlineTag.tag` (the inline-tag name) is reserved — the parser
  // currently drops it during emit, so the visitor key list is empty.
  JsdocInlineTag: [],
  JsdocGenericTagBody: ['typeSource', 'value'],
  // `JsdocBorrowsTagBody` (Kind 0x0A) and `JsdocRawTagBody` (Kind 0x0B)
  // are reserved discriminants; the parser never emits them. These keys
  // are kept for the future `@borrows` / raw-body specialization.
  JsdocBorrowsTagBody: ['source', 'target'],
  JsdocRawTagBody: [],
  JsdocParameterName: [],
  JsdocNamepathSource: [],
  JsdocIdentifier: [],
  JsdocText: [],
  // 45 TypeNode variants
  TypeName: [],
  TypeNumber: [],
  TypeUnion: ['elements'],
  TypeIntersection: ['elements'],
  TypeGeneric: ['left', 'elements'],
  TypeFunction: ['parameters', 'returnType', 'typeParameters'],
  TypeObject: ['elements']
  // ... (all 45 variants)
}
```

Whether the AST originates from JSON deserialization or from Binary AST lazy nodes, ESLint rule authors see
the same API.

## JS Public API (`parse` / `parseBatch`)

The existing `parse(text)` API is preserved for compatibility. A new `parseBatch(items)` is added for batch
processing:

```typescript
// Existing API (compatibility preserved; internal implementation is replaced with Binary AST + lazy decoder)
parse(text: string, options?: ParseOptions): ParseResult

interface Diagnostic {
  message: string  // The error/warning message body from the parser
                   // (in Binary AST: a String Offsets index → restored to a string)
}

interface ParseResult {
  ast: RemoteJsdocBlock | null  // Internally backed by the lazy decoder
  diagnostics: Diagnostic[]      // message only (no rootIndex needed since it's a single result)
  sourceFile: RemoteSourceFile   // Hold this alive while accessing `ast` getters
                                 // (lazy fields read through it)
  free?: () => void              // WASM only: release the wasm.memory.buffer
                                 // backing this result; NAPI manages the
                                 // buffer lifetime via Uint8Array refcounting
                                 // and does not expose `free`
}

// New batch API
parseBatch(items: BatchItem[], options?: ParseOptions): BatchResult

interface BatchItem {
  sourceText: string
  baseOffset?: number  // Offset within the original file (for ESLint, default 0)
                       // Added to each node's relative Pos/End to produce absolute positions
}

interface BatchResult {
  asts: (RemoteJsdocBlock | null)[]  // Corresponds to each item; null = parse failed
  diagnostics: BatchDiagnostic[]      // Across all items
  sourceFile: RemoteSourceFile        // Same lifetime contract as ParseResult.sourceFile
  free?: () => void                   // WASM only (see ParseResult.free)
}

interface BatchDiagnostic extends Diagnostic {
  rootIndex: number  // Which item (= roots[rootIndex]) it belongs to
}
```

### Wrapper internal shape

`parse()` and `parseBatch()` are thin JS wrappers over two distinct Rust
entry points (`parse_to_bytes` / `parse_batch_to_bytes`); the wrapper does
not route single-comment calls through the batch path. The wrapper's only
job is to construct the `RemoteSourceFile` from the returned bytes and
forward `diagnostics` (with `rootIndex` only on the batch side):

```typescript
function parse(text: string, options?: ParseOptions): ParseResult {
  const { buffer, diagnostics } = parseJsdocBinding(text, options)
  const sourceFile = new RemoteSourceFile(buffer, {
    emptyStringForNull: options?.emptyStringForNull
  })
  return {
    ast: sourceFile.asts[0] ?? null,
    diagnostics,
    sourceFile
    // free?: () => void  // WASM only; the WASM wrapper appends `() => handle.free()`
  }
}
```

### oxlint integration example

```typescript
const program = oxc.parse(jsCode)
const items = program.comments.map(c => ({
  sourceText: jsCode.slice(c.start, c.end),
  baseOffset: c.start
}))

const result = parseBatch(items)

// Node ranges are absolute offsets within the original jsCode
result.asts[0]?.tags[0].range // → [142, 158], etc.

// Check for parse failures
if (result.asts[1] === null) {
  const errors = result.diagnostics.filter(d => d.rootIndex === 1)
  console.log(errors[0].message) // Always at least one diagnostic exists
}
```

### Representation of parse failures

- When `RootIndexArray[i].node_index === 0` (sentinel), the JS API represents it as
  `result.asts[i] === null`
- On failure, at least one `BatchDiagnostic` always corresponds with `rootIndex === i`
- The position of the failed comment is recovered from the input `items[i]` (`baseOffset` + `sourceText.length`)

### Replacing the existing `parse()` API (at the 1.0 release)

The current `parse(text): { ast_json: String, diagnostics }` (JSON path) is **fully replaced by the
Binary AST + lazy decoder path**. No coexistence.

```typescript
// API after 1.0 (signature is preserved; the contents of the return value change)
parse(text: string, options?: ParseOptions): ParseResult

// See "JS Public API (`parse` / `parseBatch`)" above for the full ParseResult
// interface (ast / diagnostics / sourceFile / free?). Compared to the
// pre-Binary-AST shape, `ast` switches from a plain object to a
// `RemoteJsdocBlock` lazy class and the new `sourceFile` field anchors the
// underlying buffer's lifetime.
```

**The API surface is unchanged**: function signatures and property access patterns
(`result.ast.tags[0].tag`, etc.) are the same. Typical ESLint plugin code works unmodified.

**Behavioral changes (called out in release notes)**:

1. The type of `result.ast` changes from `JsdocBlock` (plain object) to `RemoteJsdocBlock` (lazy class)
2. `JSON.stringify(result.ast)` produces equivalent output via the automatic `toJSON()` invocation
   (no user changes required)
3. Non-standard code using `Object.keys(result.ast)` / `for...in` is recommended to go through
   `toPlainObject(result.ast)`
4. `result.ast.constructor.name` changes from `'Object'` to `'RemoteJsdocBlock'` (code that uses
   instanceof checks should be reviewed)
5. The new `parseBatch(items)` API is provided for large batches

**Migration timing**:

- ox-jsdoc is **pre-1.0** (the parsedType implementation just landed in PR #5) → low cost of protecting
  existing users; high tolerance for breaking changes
- Binary AST introduction is performed as a **regular release in the pre-1.0 stage** (the 1.0 release is a
  separate decision; not necessarily tied to the Binary AST introduction)
- The release notes explicitly announce the diffs 1-5 above (treated as a breaking change in a minor version
  bump within pre-1.0)

Coexistence (additional APIs such as `parseLegacy`, `parseBinary`, etc.) is **not adopted**:

- High maintenance cost of two internal paths
- Most user code works as is, so the benefit of coexistence is small
- Pre-1.0, so the tolerance for breaking changes is high
