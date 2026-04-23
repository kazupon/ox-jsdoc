# Testing Strategy

Binary AST combines a binary format, a lazy decoder, and multiple bindings (Rust + NAPI + WASM),
so it requires a richer strategy than ordinary AST testing. The 16 categories below provide complete coverage:

## Design overview

The ox-jsdoc Binary AST testing strategy is structured along three axes: **bit-perfect verification of the format
spec**, **semantic equivalence with the typed AST plus reuse of existing test assets**, and **verification of
lazy decoder characteristics**.

Key decisions:

- **16 categories ensure coverage**: unit / encoder / decoder / roundtrip / compatibility / JS-side /
  cross-binding / edge case / Visitor / memory safety / Fuzzing / Snapshot / performance / bit-level /
  lazy and cache / compat switching — each layer is verified independently
- **Maximum reuse of existing test assets** (50-70% reduction): the 7 buckets under `fixtures/perf/` and the
  existing vitest tests are reused via **import substitution**, with almost zero new code written
- **Expected JSON files**: the current typed AST output is fixed as "the baseline that guarantees the current
  state", and the Binary AST side is verified to return equivalent output (generated in bulk at the start of Phase 1.0)
- **bit-perfect verification** (category 14): roundtrip verification of encoder/decoder via **direct byte reads**
  of Common Data 6-bit / Node Data type tag / Children bitmask
- **Independent measurement of lazy characteristics** (category 15): proxy construction count hooks /
  identity caching / `EMPTY_NODE_LIST` shared singleton / Rust stack value type size assertions
- **Phased introduction per Phase**: unit tests in Phase 1.0, encoder/decoder in Phase 1.1, binding integration
  in Phase 1.2, Fuzzing just before Phase 1.3 — early failure detection

## 1. Unit tests (per module)

- `format/header.rs`: Header is 40 bytes, Major+Minor 4-bit packing works correctly
- `format/kind.rs`: single-instruction optimization checks (`is_type_node` etc.) are correct for all 256 values
- `format/node_record.rs`: 24-byte layout, offsets of each field
- `format/string_table.rs`: u16 upper bound, overflow detection
- Covered exhaustively via `#[cfg(test)] mod tests` inside each module

## 2. Encoder tests (writer → expected byte sequence)

Each `write_*` function under `crates/ox_jsdoc_binary/src/writer/` produces the expected byte sequence:

```rust
#[test]
fn write_jsdoc_block_minimal() {
    let mut writer = BinaryWriter::new();
    writer.write_jsdoc_block(/* ... */);
    let bytes = writer.finish();
    insta::assert_snapshot!(format_hex(&bytes));
}
```

`insta` snapshots detect unintended byte-sequence changes.

## 3. Decoder tests (expected byte sequence → expected values)

A fixed binary fixture is passed to the decoder, and each property is verified to return the expected value:

```rust
#[test]
fn decode_jsdoc_block_minimal() {
    let bytes = include_bytes!("fixtures/minimal_block.bin");
    let sf = LazySourceFile::new(bytes).expect("valid binary");
    let block = sf.asts().next().unwrap().expect("parse OK");
    assert_eq!(block.range(), [0, 30]);
    assert_eq!(block.tags().count(), 1);
}
```

## 4. Roundtrip tests (encode → decode, semantic equivalence)

Verify across all fixtures that the parser emits binary directly → the decoder reads it back → input
information is fully preserved:

```rust
#[test]
fn roundtrip_preserves_semantics() {
    for source in load_all_fixtures() {
        let result = parse(&allocator, &source, Default::default());
        let sf = LazySourceFile::new(result.binary_bytes).expect("valid binary");
        let decoded = sf.asts().next().unwrap().expect("parse OK");
        assert_decoded_matches_input(decoded, &source);
    }
}
```

## 5. Compatibility tests (typed AST vs binary AST, equivalence verification)

Serialize the outputs of `crates/ox_jsdoc/` (typed AST) and `crates/ox_jsdoc_binary/` to JSON and compare them.
During the Phase 1.0-1.2 coexistence period, this guarantees that both parsers return the same AST:

```rust
// crates/ox_jsdoc_binary/tests/compat_with_typed_ast.rs
#[test]
fn typed_and_binary_produce_equivalent_ast() {
    for source in load_all_fixtures() {
        let typed_json = serialize_typed_to_json(typed::parse(&source));
        let binary_json = serialize_binary_to_json(binary::parse(&source));
        assert_eq!(typed_json, binary_json, "fixture: {}", source);
    }
}
```

This runs on **the same fixtures** as the performance comparison benchmark, simultaneously guaranteeing
performance and semantic equivalence. It is the key test that proves the precondition for the Phase 1.3
cutover (binary is fully equivalent to typed).

## 6. JS-side tests (vitest, both bindings)

The same test cases run in `napi/ox-jsdoc-binary/test/` and `wasm/ox-jsdoc-binary/test/`:

```typescript
import { parse, parseBatch } from 'ox-jsdoc-binary'

it('returns lazy RemoteJsdocBlock', () => {
  const result = parse('/** @param {string} id */')
  expect(result.ast.type).toBe('JsdocBlock')
  expect(result.ast.tags[0].tag).toBe('param')
})

it('toJSON produces equivalent output to typed AST', () => {
  const binaryResult = parse('/** @param x */')
  const typedResult = parseTyped('/** @param x */')
  expect(JSON.parse(JSON.stringify(binaryResult.ast))).toEqual(typedResult.ast)
})

it('batch handles parse failures', () => {
  const result = parseBatch([{ sourceText: '/** valid */' }, { sourceText: '/* not jsdoc */' }])
  expect(result.asts[0]).not.toBeNull()
  expect(result.asts[1]).toBeNull()
  expect(result.diagnostics.filter(d => d.rootIndex === 1).length).toBeGreaterThan(0)
})
```

## 7. NAPI vs WASM consistency tests

For the same input, NAPI and WASM produce **the same byte sequence** and the **same decoded result**:

```typescript
import { getBytes as getBytesNapi } from 'ox-jsdoc-binary'
import { getBytes as getBytesWasm } from '@ox-jsdoc/wasm-binary'

it('NAPI and WASM produce identical binary output', () => {
  for (const source of fixtures) {
    expect(getBytesNapi(source)).toEqual(getBytesWasm(source))
  }
})
```

This verifies the precondition that Rust's crates/ox_jsdoc_binary uses the same code in both bindings.

## 8. Edge case tests (dedicated fixtures)

Place the following under `fixtures/edge_cases/`:

| Fixture                        | Verification target                                                                                                                            |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `empty_comment.jsdoc`          | `/** */` (minimum)                                                                                                                             |
| `single_tag.jsdoc`             | Single tag                                                                                                                                     |
| `unicode_heavy.jsdoc`          | Strings with UTF-16 surrogates (UTF-8 → UTF-16 conversion boundary)                                                                            |
| `parse_failure.jsdoc`          | Invalid JSDoc (root index = 0 sentinel)                                                                                                        |
| `large_batch_100.json`         | 100-comment batch                                                                                                                              |
| `string_table_overflow.json`   | 64K+ unique strings (overflow test)                                                                                                            |
| `compat_mode_block.jsdoc`      | compat_mode with all fields populated                                                                                                          |
| `empty_arrays.jsdoc`           | All array fields empty (verifies ED list metadata `(head=0, count=0)` correctly yields `EMPTY_NODE_LIST`)                                      |
| `deep_nesting.jsdoc`           | Deeply nested TypeNode (Union of Generic of ...)                                                                                               |
| `all_kinds.jsdoc`              | Large comment containing all 60 node kinds (`NodeList` Kind 0x7F is reserved-only and intentionally absent — encoder never emits it)           |
| `string_inline_boundary.jsdoc` | String-leaf nodes around the 256-byte boundary — verifies `TypeTag::StringInline` (≤ 255) vs `TypeTag::String` (≥ 256) selection (Path B-leaf) |

## 9. Visitor traversal tests

Verify that the `LazyJsdocVisitor` trait correctly traverses all nodes:

```rust
#[test]
fn visitor_visits_all_nodes() {
    // Hold a counter per Kind and let the default implementation handle recursive walks
    // (hook visit_xxx to count++, then descend to children via the default)
    struct CountVisitor { counts: HashMap<Kind, usize> }
    impl<'a> LazyJsdocVisitor<'a> for CountVisitor {
        fn visit_block(&mut self, b: LazyJsdocBlock<'a>) {
            *self.counts.entry(Kind::JsdocBlock).or_insert(0) += 1;
            self.visit_block_default(b);  // Recursively walk children via the default impl
        }
        fn visit_tag(&mut self, t: LazyJsdocTag<'a>) {
            *self.counts.entry(Kind::JsdocTag).or_insert(0) += 1;
            self.visit_tag_default(t);
        }
        // Hook each visit_xxx with the same pattern (can be macro-generated in Phase 4)
    }
    let result = parse(/* fixture with known node counts per Kind */);
    let mut v = CountVisitor { counts: HashMap::new() };
    v.visit_block(result.lazy_root);
    assert_eq!(v.counts[&Kind::JsdocBlock], 1);
    assert_eq!(v.counts[&Kind::JsdocTag], EXPECTED_TAG_COUNT);
    // ... derive expected values per Kind from the fixture and assert
}
```

## 10. Memory safety / rejection of malformed binary tests

The decoder must not crash on malformed binaries; it returns a clear error:

```rust
#[test]
fn rejects_invalid_version() {
    let buffer = vec![0xF0, /* ... */];  // Unknown Major
    assert!(matches!(LazySourceFile::new(&buffer),
                     Err(DecodeError::IncompatibleMajor { .. })));
}

#[test]
fn rejects_truncated_buffer() {
    assert!(matches!(LazySourceFile::new(&[0x10, 0x00]),
                     Err(DecodeError::TooShort)));
}

#[test]
fn handles_string_table_overflow() {
    let mut writer = BinaryWriter::new();
    for _ in 0..70_000 { let _ = writer.add_string("dummy"); }
    assert!(matches!(writer.finish(),
                     Err(EncodeError::StringTableOverflow { .. })));
}
```

## 11. Fuzzing (cargo-fuzz)

Pass random byte sequences to the decoder to detect crashes/UB:

```rust
// fuzz/fuzz_targets/decode_arbitrary.rs
fuzz_target!(|data: &[u8]| {
    let _ = LazySourceFile::from_bytes(data);  // Must not crash
});

// fuzz/fuzz_targets/roundtrip.rs (property-based)
fuzz_target!(|source: &str| {
    if let Ok(parsed) = parse(source) {
        let decoded = LazySourceFile::from_bytes(parsed.binary_bytes()).unwrap();
        // Each field of decoded matches parsed
    }
});
```

CI runs short (a few minutes) smoke fuzzing; longer runs are done locally.

## 12. Snapshot tests (insta + vitest)

Snapshot the encoded result of each fixture as a hex dump:

```rust
// crates/ox_jsdoc_binary/tests/snapshots.rs
#[test]
fn snapshot_typescript_checker_first_comment() {
    let bytes = encode(typescript_checker_first_comment());
    insta::assert_snapshot!(hex_dump(&bytes));
}
```

When the format spec changes, **detect unintended byte-sequence changes**. Approve/reject with
`cargo insta review`.

## 13. Performance tests (criterion + mitata)

(see [benchmark.md](./benchmark.md) for details)

- Rust side: criterion compares `crates/ox_jsdoc` vs `crates/ox_jsdoc_binary`
- JS side: mitata runs a 4-way comparison (napi/wasm × typed/binary)
- CI automates performance regression detection

## 14. Bit-level encoding verification (Common Data / Node Data / Children bitmask)

Verify that the 6-bit / 30-bit / bitmask layouts in format.md are kept **bit-perfect** by encoder/decoder.
Use minimal inputs to verify down to the single-bit level:

```rust
#[test]
fn common_data_jsdoc_tag_optional_round_trips() {
    // Roundtrip verification of Common Data bit0 = optional
    for &optional in &[false, true] {
        let bytes = encode_jsdoc_tag(Tag { name: "param", optional, /*...*/ });
        let sf    = LazySourceFile::new(&bytes).unwrap();
        let block = sf.asts().next().unwrap().unwrap();
        let tag   = block.tags().next().unwrap();
        assert_eq!(tag.optional(), optional);
        // Also verify the byte directly (makes the intent of the test explicit)
        let common_byte = bytes[sf.nodes_offset as usize + /*tag node index*/ 24 + 1];
        assert_eq!(common_byte & 0b0000_0001, optional as u8);
    }
}

#[test]
fn node_data_type_tag_dispatches_correctly() {
    // Each type tag (0b00 Children / 0b01 String / 0b10 Extended) decodes correctly
    for kind in [Kind::TypeFunction /*Children*/, Kind::TypeName /*String*/,
                 Kind::JsdocBlock /*Extended*/] {
        let bytes = encode_minimal_node(kind);
        let sf = LazySourceFile::new(&bytes).unwrap();
        let nd = read_u32(&bytes, sf.nodes_offset as usize + 24 + 12);
        let type_tag = (nd >> 30) & 0b11;
        assert_eq!(type_tag, expected_type_tag_for(kind));
    }
}

#[test]
fn children_bitmask_jsdoctag_8_bits() {
    // Cover every combination of JsdocTag Extended Data byte 0 (8 bits)
    // bit0=tag (always required=1), toggle bit1-7 0/1 for 2^7=128 cases
    for mask_low in 0..=0x7Fu8 {
        let mask = 0b1 | (mask_low << 1);  // bit0 required
        let bytes = encode_jsdoc_tag_with_bitmask(mask);
        let ext_offset = ext_offset_for(&bytes, 1);
        assert_eq!(bytes[ext_offset], mask, "mismatch for mask={:#010b}", mask);
    }
}
```

Coverage targets:

- Each node type and each bit of Common Data (6-bit) (every entry in the table at [format.md "Usage per node kind"](./format.md#per-node-kind-usage))
- All 4 Node Data type tags (`Children` / `String` / `Extended` / `StringInline`); the encoder picks `StringInline` (0b11) for short string-leaf values and `String` (0b01) only when the value exceeds the inline encoding limits (length > 255 OR offset > 4 MB-1)
- Children bitmask (JsdocTag 8 bits / JsdocBlock 3 bits / JsdocGenericTagBody 2 bits)
- u32 sentinels: `0x3FFF_FFFF` for the Node-Data `TypeTag::String` payload (None marker) and `0xFFFFFFFF` for compat line indices
- StringField NONE sentinel (`offset = 0xFFFF_FFFF, length = 0`) for absent Extended Data string slots
- `TypeTag::StringInline` boundary: round-trip strings of length 254 / 255 (inline) and 256 / 257 (fallback to `String`); also verify that String Data offsets > 4 MB fall back to `String` even for short values

## 15. Lazy / cache behavior verification

Directly measure the core of the lazy decoder (proxy is built only on access, cache hits on re-access,
unvisited nodes have zero cost):

```typescript
// JS: verify access count vs proxy construction count
it('builds child proxy only on access', () => {
  let proxyCount = 0
  const result = parse(largeFixture, { onProxyConstructed: () => proxyCount++ })
  expect(proxyCount).toBe(1) // Only the root is constructed
  void result.ast.tags // Access constructs child proxies
  expect(proxyCount).toBeGreaterThan(1)
})

it('caches lazy property on second access (identity)', () => {
  const result = parse('/** @param x */')
  const tags1 = result.ast.tags
  const tags2 = result.ast.tags
  expect(tags1).toBe(tags2) // Same reference (#internal.$tags cache hit)
})

it('returns shared EMPTY_NODE_LIST singleton for empty arrays', () => {
  const a = parse('/** desc */').ast.tags // Empty array
  const b = parse('/** other */').ast.tags // Another empty array
  expect(a).toBe(b) // Shared singleton
  expect(a.length).toBe(0)
})
```

```rust
// Rust: verify stack value type, no heap allocation (Box::new is not called)
#[test]
fn lazy_node_is_stack_value() {
    use std::mem::size_of;
    assert!(size_of::<LazyJsdocBlock>() <= 32);  // Expected 24-32 bytes
    // Verify with jemalloc/dhat that no heap allocation happens during a walk
}
```

## 16. compat_mode switching verification (Header bit0 ON/OFF)

For the same input (sourceText), confirm that compat_mode ON/OFF produces **bit-perfect different** byte sequences:

```rust
#[test]
fn compat_mode_changes_extended_data_size() {
    let source = "/** desc\n * line 2\n */";
    let basic  = encode(source, SerializeOptions { compat_mode: false, .. });
    let compat = encode(source, SerializeOptions { compat_mode: true,  .. });

    // Header bit0
    assert_eq!(basic[1] & 0x01, 0);
    assert_eq!(compat[1] & 0x01, 1);

    // JsdocBlock Extended Data size difference: basic 68 / compat 90 bytes
    // (68 = 1 bitmask + 1 padding + 8 × StringField (6B) + 3 × list metadata (6B);
    //  compat tail adds 22B for end_line + 3 × line indices + 2 × u8 flag + padding)
    let basic_sf  = LazySourceFile::new(&basic).unwrap();
    let compat_sf = LazySourceFile::new(&compat).unwrap();
    assert!(compat_sf.extended_data_size() > basic_sf.extended_data_size());
}

#[test]
fn compat_mode_round_trips_line_metadata() {
    let source = "/** desc\n * @param x\n */";
    let bytes  = encode(source, SerializeOptions { compat_mode: true, .. });
    let sf     = LazySourceFile::new(&bytes).unwrap();
    let block  = sf.asts().next().unwrap().unwrap();
    // The compat-extended portion (end_line, description_start_line, etc.) is readable
    assert_eq!(block.end_line(), Some(2));
    assert_eq!(block.description_start_line(), Some(0));
}
```

Verification targets:

- `JsdocBlock`: 68 / 90 bytes (basic = 1 bitmask + 1 padding + 8 × StringField + 3 × list metadata = 68; compat adds 22 bytes for 4 line indices + 2 × u8 flag + alignment padding)
- `JsdocTag`: 38 / 80 bytes (basic = 1 bitmask + 1 padding + 3 × StringField + 3 × list metadata = 38; compat adds 42 bytes for 7 × StringField delimiter slots)
- `JsdocDescriptionLine` / `JsdocTypeLine`: 0 / 24 bytes (switches to Extended type only when compat — 4 × StringField)
- Output sentinels (`0xFFFFFFFF` for `Option<u32>` such as `description_start_line`; `(offset 0xFFFF_FFFF, length 0)` for `StringField::NONE`)

## CI integration

```yaml
jobs:
  rust-tests:
    - cargo test --workspace # All unit / integration / roundtrip / compatibility tests
    - cargo insta accept # Snapshot verification
    - cargo bench --no-run # Confirm benchmark compilation
  js-tests:
    - pnpm test # vitest (both bindings)
    - pnpm test:cross-binding # NAPI vs WASM consistency
  fuzzing:
    - cargo fuzz run decode_arbitrary --max-total-time 60 # Short CI run
    - cargo fuzz run roundtrip --max-total-time 60
  benchmark:
    - bash tasks/benchmark/scripts/full-comparison.mjs # Performance regression
```

## Per-Phase test addition schedule

The sub-phase numbers per Phase align with the Phase composition in [phases.md](./phases.md) "crate / package
composition" (1.0a-d, 1.1a-d, 1.2a-d).

| Phase      | Tests added                                                                                                                                            |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1.0a-d     | Unit tests (1) only (type definitions, consts, skeleton build verification, scope where `unimplemented!()` is not called)                              |
| 1.1a       | Add encoder tests (2), snapshot (12), bit-level encoding verification (14), compat_mode switching verification (16) (alongside encoder implementation) |
| 1.1b       | Add Rust decoder tests (3), memory safety (10), and the Rust side of lazy / cache behavior verification (15) (alongside lazy decoder implementation)   |
| 1.1c       | Add Visitor tests (9) (alongside the LazyJsdocVisitor trait implementation)                                                                            |
| 1.1d       | Add JS-side tests (6) and the JS side of lazy / cache behavior verification (15) (`@ox-jsdoc/decoder` lazy classes, direct DataView passing)           |
| 1.2a       | (parser implementation, no new tests added — regression checked via existing tests)                                                                    |
| 1.2b-c     | Roundtrip (4), compatibility tests (5), edge cases (8), cross-binding (7) (NAPI/WASM bindings)                                                         |
| 1.2d       | Performance tests (13)                                                                                                                                 |
| Before 1.3 | Fuzzing (11)                                                                                                                                           |

## Reuse of existing test assets

ox-jsdoc has a rich set of existing test assets for the typed AST (642 Rust tests + vitest suites + fixtures).
Reuse these to the maximum extent for Binary AST testing.

### Existing test assets

```
crates/ox_jsdoc/
  tests/
    type_parse_unit.rs      ← TypeNode unit tests
    type_fixtures.rs         ← TypeNode fixture-based tests
  src/**/*.rs                ← #[cfg(test)] inline, 642 test functions in total

napi/ox-jsdoc/test/
  parse.test.ts              ← vitest for parse()
  parsed-type.test.ts        ← vitest for parsedType

wasm/ox-jsdoc/test/
  parse.test.ts              ← vitest for WASM parse()

fixtures/perf/               ← Performance fixtures (7 buckets)
  common/, description-heavy/, type-heavy/, special-tag/, malformed/,
  source/, toolchain/
```

### Reuse policy

| Existing asset                                     | Reuse for Binary AST                                                                      |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| All 7 `fixtures/perf/` buckets                     | **Fully reused** (input shared by both parsers)                                           |
| Input strings in `parse.test.ts`                   | **Copy + import substitution** works (property access is the same even with lazy classes) |
| `#[cfg(test)]` in `parser/**/*.rs` (typical cases) | Reuse the input + expected structure ideas, rewrite only the API portion                  |
| validator/analyzer tests (one each)                | Reuse the input + expected diagnostics/output                                             |
| Rust type fixtures                                 | Reuse the TypeNode input text                                                             |

### Shared test corpus

Place the expected AST as a JSON file alongside each fixture:

```
fixtures/perf/common/basic-param.jsdoc          ← Input (existing)
fixtures/perf/common/basic-param.expected.json  ← Expected output JSON (newly added)
```

Verifying that both parsers satisfy this JSON guarantees semantic equivalence:

```rust
// crates/ox_jsdoc/tests/fixture_compat.rs (newly added)
#[test]
fn typed_ast_matches_expected() {
    for (input, expected) in load_fixtures_with_expected() {
        let result = ox_jsdoc::parse(&allocator, &input, Default::default());
        assert_eq!(serialize_typed_to_json(&result.comment), expected);
    }
}

// crates/ox_jsdoc_binary/tests/fixture_compat.rs (newly added)
#[test]
fn binary_ast_matches_expected() {
    for (input, expected) in load_fixtures_with_expected() {
        let result = ox_jsdoc_binary::parse(&allocator, &input, Default::default());
        assert_eq!(serialize_binary_to_json(&result), expected);
    }
}
```

### Steps to generate the expected JSON (at the start of Phase 1.0)

The expected JSON is generated using **the current typed AST output as the baseline at the start of Phase 1.0**:

```bash
# Setup script run at the start of Phase 1.0 (one-time)
for fixture in fixtures/perf/**/*.jsdoc; do
  expected_path="${fixture%.jsdoc}.expected.json"
  cargo run --bin gen_expected_json -- "$fixture" > "$expected_path"
done
git add fixtures/perf/**/*.expected.json
```

This way:

- The typed AST side acts as **"the baseline that guarantees the current state"**
- The binary AST side is verified to **"return output equivalent to the typed AST"**

### Reusing vitest tests (just substitute the import)

```typescript
// Existing: napi/ox-jsdoc/test/parse.test.ts
import { parse } from 'ox-jsdoc'
it('parses a basic param tag', () => {
  const result = parse('/** @param {string} id - The user ID */')
  expect(result.ast.tags[0].tag).toBe('param')
})

// New: napi/ox-jsdoc-binary/test/parse.test.ts
import { parse } from 'ox-jsdoc-binary' // ← Only the import source changes
it('parses a basic param tag', () => {
  const result = parse('/** @param {string} id - The user ID */')
  expect(result.ast.tags[0].tag).toBe('param') // ← Lazy classes also work via getters
})
```

→ Most vitest tests are expected to work with **just the import statement substituted**.

### Shared test helpers

Extract common helpers imported by both parsers' tests:

```typescript
// test-utils/load-fixtures.ts (newly shared module)
export function loadFixtures(): Array<{ name: string; input: string; expected: object }> {
  // Load all fixtures and .expected.json files from fixtures/perf/
}

export function assertAstEquals(actual: unknown, expected: object) {
  // Compare via toJSON (works for both lazy classes and plain objects)
  expect(JSON.parse(JSON.stringify(actual))).toEqual(expected)
}
```

### Test cleanup at the Phase 1.3 cutover

```text
Delete:
  crates/ox_jsdoc/tests/* (typed AST version tests)
  typed AST-specific tests in napi/ox-jsdoc/test/

Keep (permanent):
  fixtures/perf/, fixtures/edge_cases/, fixtures/perf/**/*.expected.json
  → Persist as living documentation of the spec

Rename:
  napi/ox-jsdoc-binary/test/ → napi/ox-jsdoc/test/ (binary version becomes the new standard)
  wasm/ox-jsdoc-binary/test/ → wasm/ox-jsdoc/test/
```

### Expected reuse efficiency

| Item                         | Reduction method                               | Effect                   |
| ---------------------------- | ---------------------------------------------- | ------------------------ |
| Input fixtures               | Shared (both parsers reference the same files) | **100% reuse**           |
| Expected AST structure       | Expected JSON files                            | **100% shared**          |
| vitest test cases (over 80%) | Import substitution                            | **Almost zero new code** |
| Benchmark fixtures           | Fully shared                                   | **100% reuse**           |
| **Total new test code**      |                                                | **50-70% reduction**     |
