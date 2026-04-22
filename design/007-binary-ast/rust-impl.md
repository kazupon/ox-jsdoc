# Rust Implementation

The parser writes Binary AST bytes **directly into the arena**. No typed AST struct
hierarchy is built; the Binary AST is the single source of truth.

## Design overview

The Rust-side implementation of ox-jsdoc adopts a **lazy decoder architecture
centered on the Binary AST byte stream**. By placing a structure on the Rust side
that is symmetric with the JS side (RemoteSourceFile), the encoder/decoder pair
can be generated for both languages from a single Binary Format specification.

Key design decisions:

- **Approach c-1 (Parser-Integrated Binary Writer)**: The parser writes Binary AST
  bytes directly to the arena without going through a typed AST. No serialize step
  is needed, and the transferred payload is the parser output itself (see
  [refs/construction-methods.md](./refs/construction-methods.md) for details).
- **Removal of typed AST struct hierarchy**: There are no Rust structs such as
  `JsdocBlock<'a>` or `TypeNode<'a>`; instead, **lazy wrappers** like
  `LazyJsdocBlock<'a>` are built on top of the Binary AST (symmetric with the
  JS-side `RemoteJsdocBlock`).
- **Lazy nodes are stack value types** (`#[derive(Copy, Clone)]`, 24-32 bytes or
  less): This completely eliminates the heap allocation cost of `Box::new`
  (about 10x speedup in Rust walkers).
- **Build the Binary AST on an arena allocator (e.g. bumpalo)** and share it
  **zero-copy** as a NAPI Buffer / WASM memory at parser exit.
- **Code generation synchronizes Rust and JS**: From the AST schema,
  `binary_writer_generated.rs` / `lazy_decoder_generated.rs` /
  `decoder.generated.js` are emitted simultaneously.
- **Internal API symmetric with the JS decoder**: Maintain correspondences such
  as `LazySourceFile` (Rust) <-> `RemoteSourceFile` (JS),
  `ext_offset` <-> `extOffsetOf`, and `KIND_TABLE` <-> `KIND_TABLE` (js).

## Parser-Integrated Binary Writer

### Construction approach: Approach c-1 (parser writes Binary AST directly)

```text
Parser (modified to write Binary AST directly)
  └─▶ During parsing, write bytes into the arena in Binary AST format
      ├─ Call a "write function" per node kind (e.g. write_jsdoc_block, write_jsdoc_tag)
      ├─ parent index is a forward reference, so backpatch it later
      ├─ next sibling is backpatched into the previous node when the next node is written
      └─ String Table and Extended Data buffer are built in parallel

Public API:
  parse(arena, source) -> ParseResult {
    binary_bytes: &[u8],           // Binary AST bytes (for NAPI/WASM sharing)
    lazy_root: LazyJsdocBlock<'_>, // Lazy node for the Rust-side walker
    diagnostics: ...
  }
```

Note: typed AST (Rust struct hierarchies like `JsdocBlock<'a>`, `TypeNode<'a>`)
is **removed**. Instead, lazy wrappers like `LazyJsdocBlock<'a>` are built on top
of the Binary AST.

## Rust-side lazy decoder

When the Rust-side linter / semantic analyzer etc. wants to walk the AST:

```rust
let block: LazyJsdocBlock = result.lazy_root;
// LazyJsdocBlock is a thin wrapper holding a reference to the Binary AST bytes and a node index.

// Allocate LazyJsdocTag on the Rust heap on demand at access time
for tag in block.tags() {
    println!("name = {}", tag.name());  // String is also read lazily
}
// Once the walk ends and LazyJsdocTag is dropped, the heap is also freed.
```

### Lazy nodes are stack value types (no Box allocation)

Each lazy node is **a small `#[derive(Copy, Clone)]` struct (32 bytes or less)**
placed on the stack. `Box<LazyXxx>` is not used (this completely eliminates the
heap allocation cost).

Rationale:

- In a scenario like an ESLint-equivalent Rust walker traversing 100 files x 3,000
  nodes = 300,000 nodes, eliminating the `Box::new` heap allocation cost
  (~50 ns each) yields **about a 10x speedup**.
- The fields of a lazy node are minimal — `&'a [u8]` (binary reference) +
  `node_index: u32` + `&'a LazySourceFile` — and fit within 24-32 bytes.
- The cost of pass-by-value is memcpy 32 bytes ≈ 5 ns, effectively zero via
  registers.
- Adding an arena allocator (internal / thread_local) for walking is an option,
  but it is not adopted due to lifetime management complexity and reentrancy
  issues.

Implementation sketch:

```rust
#[derive(Copy, Clone)]
pub struct LazyJsdocBlock<'a> {
    bytes: &'a [u8],
    node_index: u32,
    source_file: &'a LazySourceFile<'a>,
}

impl<'a> LazyJsdocBlock<'a> {
    #[inline]
    pub fn range(&self) -> [u32; 2] {
        // Read Pos/End from the node record and add the root's base_offset
    }

    #[inline]
    pub fn description(&self) -> Option<&'a str> {
        // Get the string index from Extended Data and return a slice from String Data
        // (zero-copy)
    }

    pub fn tags(&self) -> NodeListIter<'a, LazyJsdocTag<'a>> {
        // Look at the Children bitmask to locate the NodeList for tags
        // The iterator is also implemented as a value-type struct (not a closure)
    }
}

#[derive(Copy, Clone)]
pub struct LazyJsdocTag<'a> {
    bytes: &'a [u8],
    node_index: u32,
    source_file: &'a LazySourceFile<'a>,
}

// The iterator is also a value type (struct-based, not a closure)
pub struct NodeListIter<'a, T> {
    bytes: &'a [u8],
    current_index: u32,
    source_file: &'a LazySourceFile<'a>,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T: LazyNode<'a>> Iterator for NodeListIter<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index == 0 { return None; }
        let item = T::from_index(self.bytes, self.current_index, self.source_file);
        self.current_index = /* read next sibling */;
        Some(item)
    }
}
```

### LazySourceFile (root of the decoder)

The **decoder entry point** that manages Header / Root array / String table.
It plays a role symmetric with the JS-side `RemoteSourceFile`, and all lazy
nodes obtain strings, base offset, nodeCount, etc. through it.

```rust
#[derive(Copy, Clone)]
pub struct LazySourceFile<'a> {
    bytes: &'a [u8],
    // Offsets read from the Header (Header parsing runs only once in new())
    pub compat_mode: bool,
    pub root_array_offset: u32,
    pub string_offsets_offset: u32,
    pub string_data_offset: u32,
    pub extended_data_offset: u32,
    pub diagnostics_offset: u32,
    pub nodes_offset: u32,
    pub node_count: u32,
    pub root_count: u32,
}

impl<'a> LazySourceFile<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 40 { return Err(DecodeError::TooShort); }
        // Read the 40-byte Header at once
        let version = bytes[0];
        let flags   = bytes[1];
        Ok(LazySourceFile {
            bytes,
            compat_mode:          (flags & 0x01) != 0,
            root_array_offset:    read_u32(bytes, 4),
            string_offsets_offset: read_u32(bytes, 8),
            string_data_offset:   read_u32(bytes, 12),
            extended_data_offset: read_u32(bytes, 16),
            diagnostics_offset:   read_u32(bytes, 20),
            nodes_offset:         read_u32(bytes, 24),
            node_count:           read_u32(bytes, 28),
            // sourceTextLength is optional
            root_count:           read_u32(bytes, 36),
        })
    }

    /// String Offsets[idx] -> String Data slice (zero-copy &str reconstruction).
    /// Used by string-leaf nodes (TypeTag::String payload) and the
    /// diagnostics section's `message_index`.
    #[inline]
    pub fn get_string(&self, idx: u32) -> Option<&'a str> {
        if idx == 0xFFFF || idx == 0x3FFF_FFFF { return None; }
        let so = (self.string_offsets_offset + idx * 8) as usize;
        let start = read_u32(self.bytes, so) as usize;
        let end   = read_u32(self.bytes, so + 4) as usize;
        let sd = self.string_data_offset as usize;
        // Zero-copy slice as UTF-8 (encoder guarantees valid UTF-8)
        Some(unsafe { std::str::from_utf8_unchecked(&self.bytes[sd + start .. sd + end]) })
    }

    /// `StringField` -> String Data slice (zero-copy &str reconstruction).
    /// Used by Extended Data string slots which embed `(offset, length)`
    /// directly without going through the offsets table.
    ///
    /// `None` is signalled by `(offset = 0xFFFF_FFFF, length = 0)`.
    #[inline]
    pub fn get_string_by_field(&self, field: StringField) -> Option<&'a str> {
        if field.is_none() { return None; }
        let sd = self.string_data_offset as usize;
        let start = sd + field.offset as usize;
        let end   = start + field.length as usize;
        Some(unsafe { std::str::from_utf8_unchecked(&self.bytes[start..end]) })
    }

    /// base_offset of the i-th entry of the root array
    #[inline]
    pub fn get_root_base_offset(&self, root_index: u32) -> u32 {
        let off = (self.root_array_offset + root_index * 12 + 8) as usize;
        read_u32(self.bytes, off)
    }

    /// AST for each root (None if parse failed)
    pub fn asts(&'a self) -> impl Iterator<Item = Option<LazyJsdocBlock<'a>>> + 'a {
        (0..self.root_count).map(move |i| {
            let off = (self.root_array_offset + i * 12) as usize;
            let node_idx = read_u32(self.bytes, off);
            if node_idx == 0 { None } else {
                Some(LazyJsdocBlock::from_index(self.bytes, node_idx, self))
            }
        })
    }
}

#[inline]
fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
```

### Helper functions (shared parts for reading Binary AST)

Common low-level operations called from each lazy node type. Symmetric with the
JS side (`extOffsetOf` / `childAtVisitorIndex`):

```rust
/// Node Data -> byte offset within Extended Data (Extended type 0b10 only)
/// Node Data layout: [31:30]=type tag (2 bit) + [29:0]=payload (30 bit)
#[inline]
pub fn ext_offset<'a>(sf: &LazySourceFile<'a>, node_index: u32) -> u32 {
    let byte_index = (sf.nodes_offset + node_index * 24) as usize;
    let node_data  = read_u32(sf.bytes, byte_index + 12);
    let type_tag   = (node_data >> 30) & 0b11;
    debug_assert_eq!(type_tag, 0b10, "node {} is not Extended type", node_index);
    sf.extended_data_offset + (node_data & 0x3FFF_FFFF)
}

/// Children bitmask + visitor index -> index of the corresponding child node
/// Children are placed contiguously starting right after the parent (parent_index + 1)
/// in DFS pre-order.
/// visitor index n = the n-th set bit in the bitmask.
#[inline]
pub fn child_at_visitor_index(
    sf: &LazySourceFile<'_>,
    parent_index: u32,
    bitmask: u8,
    visitor_index: u8,
) -> Option<u32> {
    let mut child = parent_index + 1;
    let mut visitor_pos = 0u8;
    for bit in 0..8 {
        if (bitmask & (1 << bit)) == 0 { continue; }  // This slot is not emitted
        if visitor_pos == visitor_index { return Some(child); }
        // Advance to the next child (= next_sibling of the current child)
        let off = (sf.nodes_offset + child * 24 + 20) as usize;
        let next = read_u32(sf.bytes, off);
        if next == 0 { return None; }                  // No-child sentinel
        child = next;
        visitor_pos += 1;
    }
    None
}
```

### Kind dispatch (Kind → type selection)

From the **Kind value (u8)** at byte 0 of the node record, look up the
corresponding handling (visitor dispatch or type conversion). Symmetric with
the JS-side `decodeKindToClass`, and **code-generated** in Phase 4 (see
ast-nodes.md):

```rust
// generated/kind_table.rs (auto-generated in Phase 4)

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Kind {
    Sentinel              = 0x00,
    JsdocBlock            = 0x01,
    JsdocDescriptionLine  = 0x02,
    JsdocTag              = 0x03,
    // ... 15 comment AST kinds
    NodeList              = 0x7F,
    TypeName              = 0x80,
    TypeNumber            = 0x81,
    // ... 45 TypeNode kinds
}

impl Kind {
    #[inline]
    pub fn from_u8(value: u8) -> Result<Self, DecodeError> {
        // Fast check using constraints on the Kind number space (ast-nodes.md "Category check implementation")
        // - TypeNode: kind & 0x80 != 0
        // - NodeList: kind == 0x7F
        // - Sentinel: kind == 0x00
        // - Comment AST: otherwise (0x01-0x0F)
        // Unused Kinds are decoder errors
        match value {
            0x00 => Ok(Kind::Sentinel),
            0x01..=0x0F => Self::from_u8_comment_ast(value),
            0x7F        => Ok(Kind::NodeList),
            0x80..=0xFF => Self::from_u8_type_node(value),
            _ => Err(DecodeError::UnknownKind(value)),
        }
    }

    #[inline]
    pub fn is_type_node(self) -> bool { (self as u8) & 0x80 != 0 }
    #[inline]
    pub fn is_node_list(self) -> bool { self as u8 == 0x7F }
    #[inline]
    pub fn is_sentinel(self) -> bool  { self as u8 == 0x00 }
}
```

Each lazy node type (such as `LazyJsdocBlock`) asserts the corresponding Kind
in the `from_index` constructor so it does not wrap a wrong node:

```rust
impl<'a> LazyJsdocBlock<'a> {
    pub fn from_index(bytes: &'a [u8], node_index: u32, sf: &'a LazySourceFile<'a>) -> Self {
        debug_assert_eq!(
            Kind::from_u8(bytes[(sf.nodes_offset + node_index * 24) as usize]).unwrap(),
            Kind::JsdocBlock
        );
        LazyJsdocBlock { bytes, node_index, source_file: sf }
    }
}
```

### Lazy Visitor trait (code generation)

The `LazyJsdocVisitor` trait is also emitted by code generation (Phase 4). Each
method **takes a lazy node by value** and recursively walks via the default
implementation:

```rust
// generated/visitor.rs (generated in Phase 4)
pub trait LazyJsdocVisitor<'a> {
    fn visit_block(&mut self, block: LazyJsdocBlock<'a>) {
        self.visit_block_default(block);
    }

    fn visit_block_default(&mut self, block: LazyJsdocBlock<'a>) {
        for tag in block.tags() {
            self.visit_tag(tag);
        }
        for line in block.description_lines() {
            self.visit_description_line(line);
        }
        // ...
    }

    fn visit_tag(&mut self, tag: LazyJsdocTag<'a>) {
        self.visit_tag_default(tag);
    }

    fn visit_tag_default(&mut self, tag: LazyJsdocTag<'a>) {
        if let Some(parsed_type) = tag.parsed_type() {
            self.visit_type_node(parsed_type);
        }
        for line in tag.description_lines() {
            self.visit_description_line(line);
        }
    }

    // ... visit_xxx methods for all 60 kinds
}

// Example: an ESLint-equivalent checker
struct UnknownTagChecker {
    errors: Vec<String>,
    known_tags: HashSet<&'static str>,
}

impl<'a> LazyJsdocVisitor<'a> for UnknownTagChecker {
    fn visit_tag(&mut self, tag: LazyJsdocTag<'a>) {
        // tag.name() returns the child node `Option<LazyJsdocTagNameValue<'a>>`
        // (see format.md JsdocTag Children bitmask bit2=name)
        let name_str = tag.name().map(|n| n.value()).unwrap_or("");
        if !self.known_tags.contains(name_str) {
            self.errors.push(format!("unknown tag: {}", name_str));
        }
        self.visit_tag_default(tag);  // Visit children too
    }
}
```

This is the same pattern as the JS-side `RemoteJsdocBlock` (class instances are
also effectively lightweight, holding a view ref + node_index). Code generation
(Phase 4) emits the Rust and JS lazy decoders + visitors from a single schema.

## Sharing with NAPI/WASM

| Layer    | Sharing approach                                                                                  |
| -------- | ------------------------------------------------------------------------------------------------- |
| **NAPI** | Pass the binary bytes on the arena to JS as a NAPI Buffer via **zero-copy reference** (no memcpy) |
| **WASM** | View the arena region directly from JS via `new Uint8Array(wasm.memory.buffer, offset, length)`   |

In both environments, no encoder step (serializing the byte stream) is required.
The Binary AST that the parser wrote into the arena is the transferred payload
itself.

## Processing flow of the Parser-Integrated Binary Writer

```text
1. Reserve 40 bytes for the Header (offsets are written back later)
2. Initialize the String Table (place each BatchItem's sourceText into String Data in order, as UTF-8)
3. For each BatchItem's sourceText, **build a UTF-8 -> UTF-16 conversion map**
   (skip optimization with an identity map if ASCII-only)
4. The parser, while reading tokens, **writes them directly into the arena in Binary AST format**.
   When each node is recognized:
   a. Determine the Kind
   b. Register string fields in the String Table (prefer zero-copy substrings, as UTF-8)
   c. Convert the node's Pos/End to **UTF-16 code unit offsets** and write into the 24-byte record
   d. Pack node-specific data into Common Data + Node Data
   e. If Extended Data is needed, append to the Extended Data buffer
   f. Append the 24-byte node record to the Nodes buffer (written as LE u32)
5. Wrap array fields in a NodeList (Kind 0x7F) just like tsgo.
   However, **for empty arrays, do not emit a NodeList and set the corresponding bit
   in the parent's Children bitmask to 0** (Option A2 optimization, reduces overhead at batch time)
6. Resolve forward references for parent-child links via backpatching:
   - parent index: At child write time, the parent's index is already known, so set it
   - next sibling: When the next node is written, patch the corresponding bytes of the previous node
7. Write the root metadata for each BatchItem (node_index, source_offset, base_offset)
   into the Root Index Array
8. Sort diagnostics in ascending root_index order and write into the Diagnostics section
9. Concatenate all sections:
   Header + RootIndexArray + StringOffsets + StringData + ExtData + Diagnostics + Nodes
10. Write back each offset in the Header (LE u32)
```

### Backpatching details (parent / next_sibling)

Because nodes are **written sequentially in DFS pre-order**, "my parent index"
is fixed at write time, but "next sibling index" is not yet determined (forward
reference). Backpatching solves this:

```rust
struct NodeWriter<'a> {
    nodes_buffer: &'a mut Vec<u8>,        // Contiguous buffer of 24 bytes/node
    next_sibling_patch: Vec<u32>,         // [parent_index] = byte offset of the child waiting for "next sibling"
}

impl<'a> NodeWriter<'a> {
    /// Emit a new node (parent index is known, next_sibling is provisionally set to 0)
    fn emit_node(&mut self, parent_index: u32, kind: u8, /*...*/) -> u32 {
        let new_index = (self.nodes_buffer.len() / 24) as u32;
        let new_byte_offset = self.nodes_buffer.len() as u32;

        // Write 24 bytes at once (next_sibling is 0 = sentinel)
        self.nodes_buffer.push(kind);                          // byte 0: Kind
        self.nodes_buffer.push(/* common data */);             // byte 1
        self.nodes_buffer.extend_from_slice(&[0, 0]);           // byte 2-3 padding
        self.nodes_buffer.extend_from_slice(&pos.to_le_bytes()); // byte 4-7
        self.nodes_buffer.extend_from_slice(&end.to_le_bytes()); // byte 8-11
        self.nodes_buffer.extend_from_slice(&node_data.to_le_bytes()); // byte 12-15
        self.nodes_buffer.extend_from_slice(&parent_index.to_le_bytes()); // byte 16-19
        self.nodes_buffer.extend_from_slice(&0u32.to_le_bytes()); // byte 20-23: next_sibling (provisional)

        // If there is a previous sibling under the same parent, patch this index into its bytes 20-23
        if let Some(prev_sibling_byte) = self.next_sibling_patch.get(parent_index as usize) {
            if *prev_sibling_byte != 0 {
                let bytes = new_index.to_le_bytes();
                self.nodes_buffer[*prev_sibling_byte as usize + 20..*prev_sibling_byte as usize + 24]
                    .copy_from_slice(&bytes);
            }
        }
        // Register self as the "node waiting for the next sibling"
        self.next_sibling_patch[parent_index as usize] = new_byte_offset;

        new_index
    }
}
```

The **parent index** does not need backpatching because the parent's index is
already known when the child is written. **next_sibling** is patched into bytes
20-23 of the previous sibling at the moment the next sibling is emitted.

### PositionMap (UTF-8 -> UTF-16 conversion)

A table that converts the UTF-8 byte offset of each sourceText to a UTF-16 code
unit offset. Equivalent to tsgo's `PositionMap`.

```rust
pub struct PositionMap {
    /// None: ASCII-only (identity map, no table needed)
    /// Some: mapping table from UTF-8 byte offset to UTF-16 code unit offset (sparse)
    table: Option<Vec<(u32, u32)>>,  // sorted array of (utf8_offset, utf16_offset)
}

impl PositionMap {
    /// Build the map by scanning sourceText once
    pub fn build(source: &str) -> Self {
        // ASCII-only fast path: if every byte is < 0x80, use the identity map
        if source.bytes().all(|b| b < 0x80) {
            return PositionMap { table: None };
        }

        // If non-ASCII characters are included: record (utf8_pos, utf16_pos) for each char
        let mut table = Vec::new();
        let mut utf8_pos = 0u32;
        let mut utf16_pos = 0u32;
        for ch in source.chars() {
            table.push((utf8_pos, utf16_pos));
            utf8_pos += ch.len_utf8() as u32;
            utf16_pos += ch.len_utf16() as u32;
        }
        table.push((utf8_pos, utf16_pos));  // sentinel (end)
        PositionMap { table: Some(table) }
    }

    /// UTF-8 byte offset -> UTF-16 code unit offset
    /// Identity (no cost) if ASCII-only; binary search O(log N) for non-ASCII
    #[inline]
    pub fn to_utf16(&self, utf8_offset: u32) -> u32 {
        match &self.table {
            None => utf8_offset,  // identity (ASCII-only)
            Some(table) => {
                let i = table.partition_point(|(u8_off, _)| *u8_off <= utf8_offset);
                table[i.saturating_sub(1)].1
            }
        }
    }
}
```

**Effect of the ASCII-only optimization**: In practice, most JSDoc comments
consist of ASCII only, so both table construction and lookup cost are zero.
`PositionMap::build` itself completes with a single full byte scan (the ASCII
check).

## Code generation

The binary writer per node kind (Rust) and the lazy decoder (Rust + JS) are
auto-generated from the AST schema. Manual maintenance cannot keep up with
60 kinds + visitor keys + the 6-bit common data table.

Schema specification:

- Fields per node kind (type, Optional/required, child or string or numeric)
- visitor order (the bit order of the Children bitmask)
- Field mapping into 6-bit common data
- Extended Data layout (with/without compat_mode)

Output:

- Rust: `binary_writer_generated.rs` (the `write_*` function for each node kind, called from the parser)
- Rust: `lazy_decoder_generated.rs` (lazy wrappers per node kind such as `LazyJsdocBlock`,
  `#[derive(Copy, Clone)]` stack value-type structs)
- Rust: `visitor_generated.rs` (the `LazyJsdocVisitor` trait, where each `visit_xxx`
  method takes a lazy node by value and recursively walks via the default implementation)
- Rust: `kind_table.rs` (Kind enum, Kind <-> name map)
- JS: `decoder.generated.js` (RemoteNode subclasses, visitor keys)
- JS: `protocol.js` (constants: Kind values, offsets, masks)

Because the Rust and JS lazy decoders are generated from the same schema, the
semantics of field access are guaranteed to remain consistent.

The JS decoder is emitted into a **shared decoder package** (e.g.
`@ox-jsdoc/decoder`), and the NAPI binding and WASM binding each import it.
Since both bindings share the format specification (LE byte order, UTF-16
positions, etc.), the same decoder classes work as-is. Only binding-specific
code (buffer acquisition, initialization, lifecycle) is implemented in
separate packages.

## Public Rust API (Phase 2 onward)

For ancillary use cases such as IPC / over-the-network / persistent caches, a
public encoder API is provided in Phase 2 onward. Because the typed AST is
already removed (Approach c-1), the input directly takes **`&str` (sourceText)**
and internally launches the parser to generate Binary AST bytes:

```rust
pub struct BatchItem<'a> {
    pub source_text: &'a str,   // sourceText for each comment (required)
    pub base_offset: u32,        // absolute offset within the original file (default 0)
}

/// Generate Binary AST per batch (internally calls parser_into_binary)
pub fn parse_to_binary<'a>(
    items: &[BatchItem<'a>],
    options: SerializeOptions,
) -> Vec<u8>

/// Re-serialize an existing lazy_root (no typed AST is involved)
pub fn reserialize_binary(bytes: &[u8]) -> Vec<u8>  // typically close to a no-op
```

Note: A use case of "encoding from typed AST to Binary AST" **does not exist**
(because, with the typed AST removed, the source of truth is always the Binary
AST byte stream). To reuse byte streams for persistent caches and the like,
simply save the parser output as-is.

The implementation just calls the internal binary writer. No implementation is
required in Phase 1.

## Public Rust API (Phase 3 onward)

For interoperability from non-JS languages (Go / Python / Rust itself, etc.),
a decoder API is also considered in Phase 3 and beyond:

```rust
pub fn decode_binary(bytes: &[u8]) -> Result<DecodedAst, DecodeError>
```

Use by ox-jsdoc itself is not assumed (typed AST / lazy decoder is sufficient),
but documenting the format specification provides a reference for other-language
implementations.

---

## Expected performance

### Items eliminated

| Current (JSON approach)              | Binary AST                                                       |
| ------------------------------------ | ---------------------------------------------------------------- |
| `serde_json::to_string` (Rust)       | Flat binary write-out (close to memcpy)                          |
| String allocation for every field    | String table + source slicing                                    |
| `JSON.parse` (JS)                    | No parsing — DataView reads                                      |
| Eager object creation for every node | Lazy on-demand property access                                   |
| JSON serialize for 45 TypeNode kinds | Integrated within the same Binary AST (no dedicated path needed) |

### Measurement plan

Run the same benchmarks on the current fixtures:

1. Rust parser standalone (baseline, already measured)
2. Rust parser + Binary AST encoding (new)
3. End-to-end JS binding via Binary AST (new)
4. Comparison with the current JSON-approach JS binding
5. Comparison with comment-parser and jsdoccomment
6. Each case of parseTypes ON/OFF
