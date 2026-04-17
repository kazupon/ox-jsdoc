# Implementation-Level Optimization Patterns

Concrete optimization patterns adopted in ox-jsdoc's parser implementation.
These are adapted from oxc_parser's performance design, tailored to ox-jsdoc's characteristics.

## Comment Parser (parser/)

### Adopted Patterns

| #   | Pattern                                                 | Applied To                               | oxc Reference                                                       |
| --- | ------------------------------------------------------- | ---------------------------------------- | ------------------------------------------------------------------- |
| 1   | LogicalLine kept small (24 bytes Copy)                  | `scanner.rs` `LogicalLine`               | oxc: Token bit-packed to register-size Copy type                    |
| 2   | MarginInfo separated into parallel array                | `scanner.rs` `ScanResult`                | oxc: Cold storage separation for data unused in hot path            |
| 3   | `is_content_empty` flag to eliminate redundant trim()   | `scanner.rs` `MarginInfo`                | oxc: Pre-computed flags on tokens to avoid downstream recomputation |
| 4   | `String::with_capacity()` pre-allocation                | `context.rs` `normalize_lines()`         | oxc: `vec_with_capacity()` pre-sizing                               |
| 5   | `&'a str` zero-copy slices                              | Entire parser — slices from source text  | oxc: Borrowed-slice-first, normalization in later layers            |
| 6   | Arena allocator (`oxc_allocator`)                       | Entire parser — `ArenaBox`, `ArenaVec`   | oxc: Single arena, O(1) bulk deallocation                           |
| 7   | DescLineRange index-based reference                     | `context.rs` `partition_sections()`      | oxc: Avoid copies when reference is sufficient                      |
| 8   | Skip empty lines to avoid unnecessary arena allocations | `context.rs` `parse_description_lines()` | oxc: Keep common-case fast path short                               |

### Patterns Not Yet Adopted (future consideration)

| #   | Pattern                             | Reason                                                                            |
| --- | ----------------------------------- | --------------------------------------------------------------------------------- |
| 1   | `#[inline]` / `#[cold]` annotations | Comment parser hot path is currently fast enough. Add when measurement shows need |
| 2   | Token bit-packing                   | Comment parser is line-based, not token-based                                     |
| 3   | `match` jump tables                 | Comment parser has few branches; if/else is sufficient                            |

## Type Parser (type_parser/ + parser/type_parse.rs)

### Planned Patterns

The type parser uses Pratt parsing and directly adapts these patterns
from oxc_parser's expression parser:

| #   | Pattern                                     | Applied To                                                            | oxc Reference                                            |
| --- | ------------------------------------------- | --------------------------------------------------------------------- | -------------------------------------------------------- |
| 1   | `match` jump table for O(1) dispatch        | `parse_prefix_type()`, `cur_infix_precedence()`, `parse_infix_type()` | oxc: `parse_primary_expression()` Kind match             |
| 2   | Token as Copy type (12 bytes) bit-packed    | `Token { start, end, kind }`                                          | oxc: Token as u128 bit-packed                            |
| 3   | `token_text()` lazy retrieval on demand     | Most tokens need only `kind`                                          | oxc: `cur_src()` on demand only                          |
| 4   | `#[inline]` for hot path functions          | `cur_kind()`, `bump()`, `at()`, `is_jsdoc()`                          | oxc: `cur_kind()`, `bump_any()`, `at()` with `#[inline]` |
| 5   | `#[cold] #[inline(never)]` for error paths  | `error_no_prefix()`, `error_expected()`                               | oxc: `handle_expect_failure()` with `#[cold]`            |
| 6   | `ArenaVec::with_capacity()` pre-sized       | Union elements, Generic parameters, Object fields                     | oxc: `vec_with_capacity()`                               |
| 7   | Ownership chains avoid Clone                | `left` move semantics                                                 | oxc: Expression ownership moves                          |
| 8   | Mode branches as `#[inline]` bool functions | `is_jsdoc()`, `is_typescript()`                                       | oxc: Context flag `#[inline]` checks                     |
| 9   | Zero indirect calls                         | No function pointers/vtables, all direct method calls                 | oxc: match-based direct dispatch                         |
| 10  | Absolute span offsets                       | `base_offset` added once in Lexer                                     | oxc: `base_offset` pattern                               |

### Patterns Shared by Both Parsers

| Pattern               | Comment Parser                                     | Type Parser                          |
| --------------------- | -------------------------------------------------- | ------------------------------------ |
| Arena allocator       | Same `&'a Allocator` instance                      | Same instance (via `self.allocator`) |
| Zero-copy             | `&'a str` slices from source                       | `&'a str` slices from source         |
| Pre-sized capacity    | `with_capacity()`                                  | `with_capacity()`                    |
| Error accumulation    | `Vec<OxcDiagnostic>` push, no access on happy path | Same `self.diagnostics`              |
| Absolute Span offsets | `base_offset` on ParserContext                     | `base_offset` on Lexer               |

## Reference Points from oxc_parser

### Design Principles Referenced

1. **Parser / semantic separation**: Parse phase handles structure recognition only; strict validation in later layers
2. **Compact layout**: Keep common-case nodes small
3. **Borrowed-slice-first**: Normalization in later layers; parser holds slices
4. **Fast path / cold path separation**: Keep common-case path short
5. **Generated code for structural repetition**: Prevent visitor/serializer drift

### Implementation Techniques Referenced

1. **Token bit-packing**: Fit data into register size
2. **`#[inline]` / `#[cold]`**: Maximize CPU instruction cache efficiency
3. **`match` jump tables**: O(1) dispatch
4. **Checkpoint / Rewind**: Lightweight rollback for speculative parsing
5. **Context flags**: Zero-cost state switching via bitmask

### Implementation Techniques NOT Referenced

1. **u128 Token bit-packing**: ox-jsdoc Token has fewer flags; 12 bytes is sufficient
2. **Token pre-allocation**: Comment parser is not token-based
3. **Re-lexing**: Type parser grammar has less ambiguity than JSX
