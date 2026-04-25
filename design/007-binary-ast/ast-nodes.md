# AST Nodes (Catalog of target nodes)

ox-jsdoc has two AST families — **comment AST (15 kinds)** and **TypeNode AST
(45 kinds)** — for a total of 60 kinds, plus 1 structural special node
(`Sentinel`) and 1 reserved-only discriminant (`NodeList`, kept for legacy
buffer compatibility but **never emitted by the encoder** — variable-length
child lists are now expressed via inline `(head_index, count)` metadata in
the parent's Extended Data block), handling **62 discriminants in total**
within a single Kind space (u8, 0x00-0xFF).

## Design overview

The Binary AST **Kind number space** is designed to provide growth room for
adding nodes while letting hot paths (TypeNode check / NodeList check /
Sentinel check) execute in a single instruction.

Main design goals:

- **Fit all nodes in u8 (256 slots)**: tsgo uses u32, but ox-jsdoc fits well
  within u8 with 60 + 2 kinds. Assumes the node record size fits in 24 bytes
- **Cluster TypeNode in MSB-set range (0x80-0xFF)**: the most frequent TypeNode
  check can run in a **single instruction** with `(kind & 0x80) != 0` (hot path
  for ESLint plugins / Rust walkers)
- **Reserve 0x7F as the `NodeList` discriminant — never emitted**: kept on
  the **boundary** between TypeNode (upper half) and comment AST (lower half)
  so the slot stays clearly visible in debug output. The encoder no longer
  emits NodeList wrappers (variable-length lists use inline ED metadata
  `(head_index, count)` at known per-Kind byte offsets); the discriminant is
  retained purely for legacy-buffer compatibility
- **Pin Sentinel to 0x00**: dedicated to `node[0]`, used so that
  `parent_index = 0` / `next_sibling = 0` mean "no link" (see the format.md
  Nodes section)
- **Sufficient growth room**: TypeNode has 83 spare slots / comment AST has 48
  spare slots / globally reserved 63 slots (for new categories). Persistence
  also guaranteed by the protobuf-style fixed numbering policy
- **Expand variant-bearing enums into independent Kinds**: expanding Rust's
  `JsdocTagBody` (3 variants) / `JsdocTagValue` (4 variants) into separate
  Kinds minimizes decoder dispatch cost and maintains ESTree compatibility
  (variant names visible via `node.type`)

Notes:

- `JsdocSeparator` (an enum with only `Dash`) is not made a node; it is
  embedded as a bit in the parent `JsdocGenericTagBody`'s Common Data
  (described later).
- The Rust internal `JsdocType` enum (Parsed/Raw) does not get a wrapper in
  the Binary AST; instead, `JsdocTag.parsedType` directly points to a TypeNode
  (one of Kind 0x80-0xFF) (1:1 correspondence with the existing JSON output).

## Comment AST (`crates/ox_jsdoc/src/ast.rs`)

| #   | Kind                   | Main fields                                                                                                                                                                                  | Notes                        |
| --- | ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| 1   | `JsdocBlock`           | `description?`, `tags[]`, `description_lines[]`, `inline_tags[]`, delimiter group (7), line index group (6, for compat)                                                                      | Root                         |
| 2   | `JsdocDescriptionLine` | `description`, delimiter group (3: `delimiter`, `post_delimiter`, `initial`)                                                                                                                 |                              |
| 3   | `JsdocTag`             | `tag`, `raw_type?`, `parsed_type?`, `name?`, `optional`, `default_value?`, `description?`, `raw_body?`, `body?`, `type_lines[]`, `description_lines[]`, `inline_tags[]`, delimiter group (7) | 20 fields                    |
| 4   | `JsdocTagName`         | `value`                                                                                                                                                                                      |                              |
| 5   | `JsdocTagNameValue`    | `raw`                                                                                                                                                                                        |                              |
| 6   | `JsdocTypeSource`      | `raw`                                                                                                                                                                                        | Text inside `{...}`          |
| 7   | `JsdocTypeLine`        | `raw_type`, delimiter group (3: `delimiter`, `post_delimiter`, `initial`)                                                                                                                    |                              |
| 8   | `JsdocInlineTag`       | `namepath_or_url?`, `text?`, `format`, `raw_body?`                                                                                                                                           | `tag` is reserved (see note) |
| 9   | `JsdocGenericTagBody`  | `type_source?`, `value?`, `separator?`, `description?`                                                                                                                                       |                              |
| 10  | `JsdocBorrowsTagBody`  | `source`, `target`                                                                                                                                                                           | Reserved (see note)          |
| 11  | `JsdocRawTagBody`      | `raw`                                                                                                                                                                                        | Reserved (see note)          |
| 12  | `JsdocParameterName`   | `path`, `optional`, `default_value?`                                                                                                                                                         | `JsdocTagValue::Parameter`   |
| 13  | `JsdocNamepathSource`  | `raw`                                                                                                                                                                                        | `JsdocTagValue::Namepath`    |
| 14  | `JsdocIdentifier`      | `name`                                                                                                                                                                                       | `JsdocTagValue::Identifier`  |
| 15  | `JsdocText`            | `value`                                                                                                                                                                                      | `JsdocTagValue::Raw`         |

Notes: the Rust enums `JsdocTagBody` (Generic|Borrows|Raw) and `JsdocTagValue`
(Parameter|Namepath|Identifier|Raw) are expanded so that **each variant becomes
an independent Kind** (see "Handling nodes with variants" below).

**Reserved Kinds (currently never emitted by the parser):**

- **`JsdocInlineTag.tag`** — the inline-tag name (`link`, `tutorial`, …) is
  parsed but **not preserved in the binary AST**. The Kind 0x04
  (`JsdocTagName`) record that the spec previously placed as a child of
  `JsdocInlineTag` is intentionally dropped during emit. Consumers that
  need the inline-tag name should derive it from the source `range`
  rather than the AST.
- **`JsdocBorrowsTagBody` (Kind 0x0A)** — the typed AST defines a
  `JsdocTagBody::Borrows` variant for `@borrows source as target`, but no
  parser path currently produces it; `@borrows` bodies are emitted as
  `JsdocGenericTagBody`. Kind 0x0A and the `RemoteJsdocBorrowsTagBody`
  decoder class are kept reserved for a future `@borrows` specialization.
- **`JsdocRawTagBody` (Kind 0x0B)** — same status: `JsdocTagBody::Raw` is
  defined but never produced. Reserved for future specialization.

`JsdocType` (Parsed|Raw) does not get a wrapper; instead, `JsdocTag.parsedType`
points directly to a TypeNode (rationale: 1:1 correspondence with the existing
JSON output).

## TypeNode AST (`crates/ox_jsdoc/src/type_parser/ast.rs`)

The 45 `TypeNode` variants are each treated as an independent Kind:

```text
Basic (7):       Name, Number, StringValue, Null, Undefined, Any, Unknown
Compound (7):    Union, Intersection, Generic, Function, Object, Tuple, Parenthesis
Name path (2):   NamePath, SpecialNamePath
Modifier (4):    Nullable, NotNullable, Optional, Variadic
TS-specific (11): Conditional, Infer, KeyOf, TypeOf, Import, Predicate,
                  Asserts, AssertsPlain, ReadonlyArray, TemplateLiteral, UniqueSymbol
JSDoc/Closure (1): Symbol
Supplementary (11): ObjectField, JsdocObjectField, KeyValue, Property,
                    IndexSignature, MappedType, TypeParameter, CallSignature,
                    ConstructorSignature, MethodSignature, IndexedAccessIndex
Intermediate (2):  ParameterList, ReadonlyProperty
```

Small enums attached to TypeNodes (storable in 6-bit common data):

| enum               | variant count | bits required |
| ------------------ | ------------- | ------------- |
| `ModifierPosition` | 2             | 1             |
| `GenericBrackets`  | 2             | 1             |
| `QuoteStyle`       | 2             | 1             |
| `ObjectSeparator`  | 5             | 3             |
| `NamePathType`     | 4             | 2             |
| `SpecialPathType`  | 3             | 2             |
| `VariadicPosition` | 2             | 1             |

## Kind number space

60 kinds + Sentinel + 1 reserved-only `NodeList` discriminant = 62 discriminants.
We partition `u8` (0-255) as follows so that **the hot-path TypeNode check
completes in a single instruction**:

```text
0x00         Sentinel               (1)
0x01 - 0x3F  Comment AST            (63 slots: 15 kinds + 48 spare)
0x40 - 0x7E  Globally reserved      (63 slots, for new categories)
0x7F         NodeList               (1, reserved-only boundary slot — never emitted)
0x80 - 0xFF  TypeNode               (128 slots: 45 kinds + 83 spare)
```

Design decisions:

- **Place TypeNode in 0x80-0xFF (upper bit set)**: the hot-path TypeNode check
  becomes a single MSB test (`(kind & 0x80) != 0`), compiling to a single
  instruction such as x86 `TEST AL, 0x80` / ARM `TST W0, #0x80`
- **TypeNode 83 spare slots**: leaves **more than double** the growth room for
  TypeScript spec additions (`satisfies`, `using`, decorator metadata, etc.)
  and jsdoc-type-pratt-parser extensions
- **Comment AST 48 spare slots**: leaves room for Markdown extensions
  (headings, lists, tables, etc.), Diagnostic-related additions, multi-comment
  containers, and so on
- **Reserve 0x7F as the NodeList slot — kept for legacy buffer compatibility but never emitted**: variable-length child lists are stored as inline `(head_index, count)` metadata at known per-Kind byte offsets in the parent's Extended Data block (NodeList-wrapper-elimination format change). The discriminant is reserved on the boundary between globally reserved and TypeNode (also clear in debug)

### Category check implementation

Hot paths (TypeNode check, NodeList check) complete in a **single
instruction**:

```rust
// TypeNode (most frequent): 1 instruction (MSB test)
#[inline]
fn is_type_node(kind: u8) -> bool {
    kind & 0x80 != 0   // or `kind >= 0x80` (LLVM optimizes to an equivalent instruction)
}

// NodeList: 1 instruction (compare)
#[inline]
fn is_node_list(kind: u8) -> bool {
    kind == 0x7F
}

// Sentinel: 1 instruction (compare)
#[inline]
fn is_sentinel(kind: u8) -> bool {
    kind == 0x00
}

// Comment AST: 2 instructions (auxiliary, not hot)
#[inline]
fn is_comment_ast(kind: u8) -> bool {
    (kind & 0xC0) == 0x00 && kind != 0x00
}

// Globally reserved: 2 instructions (auxiliary, not hot)
#[inline]
fn is_reserved(kind: u8) -> bool {
    (kind & 0xC0) == 0x40 && kind != 0x7F
}
```

The JS side uses the same pattern:

```javascript
const isTypeNode = kind => (kind & 0x80) !== 0
const isNodeList = kind => kind === 0x7f
const isSentinel = kind => kind === 0x00
const isCommentAst = kind => (kind & 0xc0) === 0x00 && kind !== 0x00
const isReserved = kind => (kind & 0xc0) === 0x40 && kind !== 0x7f
```

### Stability of Kind values (protocol compatibility)

**Embed explicit numbers** in the schema; once assigned, the number is fixed
(retained as a gap even if removed):

```rust
// Example AST schema (number explicit via attribute)
#[node(kind = 0x01)]
struct JsdocBlock { ... }

#[node(kind = 0x02)]
struct JsdocDescriptionLine { ... }

#[node(kind = 0x80)]
enum TypeNode {
    #[variant(kind = 0x80)] Name(TypeName),
    #[variant(kind = 0x81)] Number(TypeNumber),
    // ...
}
```

- Changing an assigned number is **strictly forbidden** (CI checks the diff)
- Removed node kinds are kept as gaps (e.g. `// reserved 0x05 (former JsdocXxx)`)
- New additions are assigned to the spare slots at the end of the category
  (e.g., new comment AST starts at 0x10, new TypeNode starts at 0xAD)
- Same proven policy as protobuf and tsgo

### Code generation for the dispatch table (Phase 4)

Category checks can be done with bitmasks, but lookups for node names, visitor
keys, etc., emit a code-generated **256-entry table**:

```rust
// generated/kind_table.rs
pub const KIND_NAMES: [&str; 256] = [
    "Sentinel",        // 0x00
    "JsdocBlock",      // 0x01
    // ...
    "",                // gap (empty string)
    // ...
    "NodeList",        // 0x7F (reserved-only — encoder never emits this kind)
    "TypeName",        // 0x80
    // ...
];

pub const VISITOR_KEYS_TABLE: [&'static [&'static str]; 256] = [
    &[],                                              // Sentinel
    &["descriptionLines", "tags", "inlineTags"],      // JsdocBlock
    // ...
];
```

The JS side generates an equivalent table in `protocol.js` etc. `KIND_NAMES[kind]`
can be retrieved with a single memory load (cacheline-friendly 256-byte array).
