# ox-jsdoc AST jsdoccomment Compatibility

## Strategy

### Strategy B: AST compatibility only, delegate runtime to jsdoccomment

ox-jsdoc's responsibility is limited to **parse speed and AST shape compatibility** only.
The runtime layer required for eslint-plugin-jsdoc operation (comment attachment, type parser,
stringify, fixer, esquery integration) is provided by jsdoccomment as-is.

```
eslint-plugin-jsdoc
  |
  v
jsdoccomment (runtime layer unchanged)
  +-- getJSDocComment()        <-- ESLint SourceCode API
  +-- commentHandler()         <-- esquery integration
  +-- estreeToString()         <-- comment reconstruction
  +-- parse/tryParse/traverse  <-- jsdoc-type-pratt-parser
  +-- stringify/rewireSpecs    <-- comment-parser derived utilities
  |
  +-- parseComment()           <-- internal parser is swappable
        +-- current: comment-parser -> commentParserToESTree()
        +-- future:  ox-jsdoc (compat_mode) used directly
```

If ox-jsdoc's `compat_mode` output matches jsdoccomment's AST shape,
simply swapping jsdoccomment's `parseComment()` internal implementation with ox-jsdoc
will allow all eslint-plugin-jsdoc rules (type parser dependent, fixer, comment attachment included)
to work as-is.

### Scope of This Strategy

**ox-jsdoc is responsible for:**

- High-performance JSDoc parsing (Rust/WASM)
- jsdoccomment-compatible AST shape output (compat_mode)
- source/tokens array reconstruction (Phase 6)

**jsdoccomment is responsible for (unchanged):**

- `getJSDocComment()` -- comment attachment via ESLint's SourceCode API
- `commentHandler()` -- JSDoc AST querying via esquery
- `estreeToString()` -- AST to string reconstruction
- `parse()`/`tryParse()`/`traverse()`/`stringify()` -- jsdoc-type-pratt-parser integration
- `rewireSpecs()`/`seedTokens()` -- comment-parser derived utilities
- source/tokens in-place mutation + stringify -- fixer workflow

### Out of Scope

- Porting eslint-plugin-jsdoc rule implementations
- Reimplementing jsdoccomment's full public API
- Type parser (jsdoc-type-pratt-parser equivalent) implementation
- Comment attachment (getJSDocComment equivalent) implementation
- Comment string reconstruction (estreeToString equivalent) implementation
- Fixer mutation + stringify workflow implementation

## Context

ox-jsdoc's AST output matches `@es-joy/jsdoccomment`'s node type names and basic structure,
but there are differences in field presence/absence that prevent direct parser swapping.

Given that benchmarks show JS binding (NAPI) performance is roughly on par with comment-parser,
implementing a jsdoccomment compatibility mode as part of AST stabilization
ahead of future Binary AST introduction.

User decisions:

- Compatibility fields are toggled via the `compat_mode` option (default preserves current behavior)
- ox-jsdoc-specific fields are excluded when `compat_mode` is enabled
- `parsedType` (type expression AST) is out of scope

## Files Changed

- `crates/ox_jsdoc/src/parser/scanner.rs` -- Add margin metadata to LogicalLine
- `crates/ox_jsdoc/src/parser/context.rs` -- Propagate margin data, track line indices
- `crates/ox_jsdoc/src/ast.rs` -- Add new fields to JsdocBlock/JsdocTag
- `crates/ox_jsdoc/src/serializer/json.rs` -- Introduce SerializeOptions, compat_mode support
- `crates/ox_jsdoc/src/lib.rs` -- Export SerializeOptions, change serialize_comment_json signature
- `napi/ox-jsdoc/src/lib.rs` -- Add JsSerializeOptions
- `wasm/ox-jsdoc/src/lib.rs` -- Add serialize_options argument
- `napi/ox-jsdoc/src-js/index.d.ts` -- Update type definitions
- `wasm/ox-jsdoc/src-js/index.d.ts` -- Update type definitions

## Phase 1: Source-Preserving Field Extraction and Output

Current state: scanner.rs strips the JSDoc margin (whitespace before `*`, the `*` itself,
space after `*`) and passes content to context.rs, but the stripped information is discarded.
The Rust AST has fields like delimiter, but they contain hardcoded values.
The serializer does not output them.

### Step 1.1: scanner.rs -- Add Margin Metadata to LogicalLine

Add the following to the `LogicalLine` struct:

```rust
pub initial: &'a str,        // indentation before *
pub delimiter: &'a str,      // the * itself (or "")
pub post_delimiter: &'a str, // space after *
pub line_end: &'a str,       // line ending (\n, \r\n, or "")
```

Capture slices of the currently-skipped portions in the margin stripping loop
of `logical_lines()`. No algorithm change, only slice capture additions.

### Step 1.2: context.rs -- Propagate Margin Data to AST

- `parse_description_lines()`: Propagate LogicalLine's initial/delimiter/post_delimiter
  to JsdocDescriptionLine (replacing current hardcoded `""`, `"*"`, `" "`)
- `parse_jsdoc_tag()`: Get delimiter/post_delimiter/initial/line_end from the tag's first line
- `parse_comment()`: Detect JsdocBlock's delimiter_line_break (whether `/**` is followed by a newline)
  and preterminal_line_break (whether `*/` is preceded by a newline)

### Step 1.3: ast.rs -- Add New Fields to JsdocBlock

```rust
pub initial: &'a str,
pub delimiter_line_break: &'a str,
pub preterminal_line_break: &'a str,
```

Add new fields to JsdocTag:

```rust
pub initial: &'a str,
pub line_end: &'a str,
```

### Step 1.4: serializer/json.rs -- Introduce SerializeOptions

New struct:

```rust
pub struct SerializeOptions {
    pub compat_mode: bool,            // output jsdoccomment-compatible fields
    pub empty_string_for_null: bool,  // null -> "" conversion
    pub include_positions: bool,      // output start/end/range (default: true)
}
```

When `compat_mode: true`:

- JsdocBlock: output delimiter, postDelimiter, initial, terminal, lineEnd,
  delimiterLineBreak, preterminalLineBreak
- JsdocTag: output delimiter, postDelimiter, postTag, postType, postName,
  initial, lineEnd
- Exclude ox-jsdoc-specific fields: optional, defaultValue, rawBody, body
- JsdocInlineTag: exclude rawBody, map format "unknown" to "plain"

**JsdocTag delimiter/postDelimiter/initial edge cases:**

In jsdoccomment, these fields conditionally become empty strings
(commentParserToESTree.js lines 380-382):

```javascript
initial: endLine ? init : '',           // empty for single-line comments
postDelimiter: lastDescriptionLine ? pd : '',  // empty when tag is on opening line
delimiter: lastDescriptionLine ? de : '',      // empty when tag is on opening line
```

| Case                                                          | delimiter | postDelimiter | initial |
| ------------------------------------------------------------- | --------- | ------------- | ------- |
| `/** @param name */` (single-line, endLine=0)                 | `""`      | `""`          | `""`    |
| `/**\n@param name\n*/` (tag on line 1, lastDescriptionLine=1) | `""`      | `""`          | `" "`   |
| Normal multi-line (description precedes tag)                  | `"*"`     | `" "`         | `" "`   |

When `lastDescriptionLine` is `0` (JS falsy value), delimiter/postDelimiter
become empty strings. The serializer must reproduce this condition.

`compat_mode: false` (default): same output as current

When `empty_string_for_null: true`:

- rawType, name, namepathOrURL, text: None -> `""` conversion

Replace existing From trait of serialize_comment_json with a `new(block, opts)` method
to accept SerializeOptions.

## Phase 2: Line Index Metadata

### Step 2.1: ast.rs -- Add Line Index Fields to JsdocBlock

```rust
pub end_line: u32,
pub description_start_line: Option<u32>,
pub description_end_line: Option<u32>,
pub last_description_line: Option<u32>,
pub has_preterminal_description: u8,        // 0 or 1
pub has_preterminal_tag_description: Option<u8>, // Some(1) or None
```

### Step 2.2: context.rs -- Line Index Computation

Within parse_comment():

- Track LogicalLine indices (0-based, `/**` line is 0)
- `description_start_line`: index of the first non-empty block description line
- `description_end_line`: index of the last non-empty block description line
- `last_description_line`: **index of the first tag line or end line**
  (Despite the name, this represents "description boundary" not "last description line".
  In jsdoccomment's implementation, it stores the idx of the first line where `tag || end` appears.
  When the value is `0`, it becomes JS falsy, causing JsdocTag's delimiter/postDelimiter
  to become empty strings -- note this edge case.)
- `end_line`: total LogicalLine count - 1
- `has_preterminal_description`: 1 when block description text exists on the `*/` line
  (when `*/` line has description and no lastTag is active)
- `has_preterminal_tag_description`: 1 when tag description text exists on the `*/` line
  (when `*/` line has description and lastTag is active,
  or when tag and end are on the same line)

### Step 2.3: serializer/json.rs -- Output Line Metadata in compat_mode

Add conditional fields to SerBlock. Serialize only when compat_mode is enabled.

## Phase 3: null vs Empty String Convention

Implemented via `empty_string_for_null` in Phase 1's SerializeOptions.

Affected fields:

- JsdocTag: rawType, name
- JsdocInlineTag: namepathOrURL, text

The Rust AST side always uses Option (no change). Conversion happens only in the serializer.

## Phase 4: Visitor Keys Export

### Step 4.1: Define Visitor Keys in JS Packages

In napi/ox-jsdoc/src-js/ and wasm/ox-jsdoc/src-js/:

```typescript
export const jsdocVisitorKeys = {
  JsdocBlock: ['descriptionLines', 'tags', 'inlineTags'],
  JsdocTag: ['parsedType', 'typeLines', 'descriptionLines', 'inlineTags'],
  JsdocDescriptionLine: [],
  JsdocTypeLine: [],
  JsdocInlineTag: []
}
```

No Rust changes needed. Only add to JS package exports.

## Phase 5: InlineTag Format Adjustment

When compat_mode is enabled:

- Map `JsdocInlineTagFormat::Unknown` to `"plain"`
- Exclude `rawBody` field

## Phase 6: comment-parser Compatible source/tokens Array Reconstruction

### Background

Many eslint-plugin-jsdoc rules (57+) depend not only on jsdoccomment's ESTree AST but also
directly on comment-parser's `jsdoc.source[]` array and `source[].tokens` objects.
Formatting rules (15+) in particular mutate tokens fields directly to implement fixes,
and eslint-plugin-jsdoc rules will not work without this structure.

### source/tokens Structure Overview

comment-parser's `source` array holds the entire comment as **one entry per line**:

```javascript
source: [
  { number: 0, source: "/**",                             tokens: { start, delimiter, postDelimiter, tag, postTag, type, postType, name, postName, description, end, lineEnd } },
  { number: 1, source: " * Description here",             tokens: { ... } },
  { number: 2, source: " * @param {string} name - desc",  tokens: { ... } },
  { number: 3, source: " */",                              tokens: { ... } },
]
```

`JsdocTag` also has `tag.source[]` containing a subset of lines belonging to that tag.

### Mapping to Current ox-jsdoc AST

After Phase 1 completion, the following information will be available in ox-jsdoc's arena AST:

| source[].tokens field | ox-jsdoc AST source                                                  |
| --------------------- | -------------------------------------------------------------------- |
| `start` (line indent) | `JsdocDescriptionLine.initial` / `JsdocTag.initial` (Phase 1)        |
| `delimiter`           | `.delimiter`                                                         |
| `postDelimiter`       | `.post_delimiter`                                                    |
| `tag`                 | `JsdocTag.tag.value` (first tag line only)                           |
| `postTag`             | `JsdocTag.post_tag` (first tag line only)                            |
| `type`                | `JsdocTag.raw_type` (first tag line only) / `JsdocTypeLine.raw_type` |
| `postType`            | `JsdocTag.post_type` (first tag line only)                           |
| `name`                | `JsdocTag.name` (first tag line only)                                |
| `postName`            | `JsdocTag.post_name` (first tag line only)                           |
| `description`         | `.description`                                                       |
| `end`                 | `JsdocBlock.terminal` (final line only)                              |
| `lineEnd`             | `JsdocBlock.line_end` / `JsdocTag.line_end` (Phase 1)                |

**Conclusion: No arena AST structural changes needed. All information is available.**

### Implementation Approach: Reconstruction in the Serializer

When `compat_mode: true`, the serializer **reconstructs** the `source[]` array
from arena AST fields.

```
Arena AST (unchanged)
  |
  v
Serializer (compat_mode only)
  |
  +-- JsdocBlock.delimiter etc -> source[0] (opening line /**)
  +-- description_lines[i]     -> source[1..n] (description lines)
  +-- JsdocTag fields          -> source[n+1] (reconstruct first tag line)
  |     tag, postTag, rawType, postType, name, postName, description
  +-- tag.description_lines    -> source[n+2..] (tag continuation lines)
  +-- tag.type_lines           -> source[...] (type continuation lines)
  +-- JsdocBlock.terminal etc  -> source[last] (closing line */)
```

Each `source` entry has a `number` (0-based line number) and a `tokens` object.
The `source` string is generated by concatenating all tokens fields.

### Step 6.1: Add Source Array Construction Logic to Serializer

Add a `build_source_array()` function to `serializer/json.rs`:

```rust
fn build_source_array(block: &JsdocBlock<'_>) -> Vec<SerSourceLine> {
    let mut lines = Vec::new();
    let mut line_number = 0u32;

    // Opening line: /**
    lines.push(build_opening_line(block, line_number));
    line_number += 1;

    // Description lines
    for desc_line in block.description_lines.iter() {
        lines.push(build_description_source_line(desc_line, line_number));
        line_number += 1;
    }

    // Tag lines
    for tag in block.tags.iter() {
        // First tag line (tag, type, name, description on one line)
        lines.push(build_tag_first_line(tag, line_number));
        line_number += 1;

        // Type continuation lines
        for type_line in tag.type_lines.iter().skip(1) { // first line already handled above
            lines.push(build_type_continuation_line(type_line, line_number));
            line_number += 1;
        }

        // Description continuation lines
        for desc_line in tag.description_lines.iter() {
            lines.push(build_description_source_line(desc_line, line_number));
            line_number += 1;
        }
    }

    // Closing line: */
    lines.push(build_closing_line(block, line_number));

    lines
}
```

### Step 6.2: SerSourceLine / SerTokens Definition

Match comment-parser's field names (camelCase):

```rust
#[derive(Serialize)]
struct SerSourceLine {
    number: u32,
    source: String,  // original text reconstructed by concatenating tokens
    tokens: SerTokens,
}

#[derive(Serialize)]
struct SerTokens {
    start: String,
    delimiter: String,
    #[serde(rename = "postDelimiter")]
    post_delimiter: String,
    tag: String,
    #[serde(rename = "postTag")]
    post_tag: String,
    r#type: String,
    #[serde(rename = "postType")]
    post_type: String,
    name: String,
    #[serde(rename = "postName")]
    post_name: String,
    description: String,
    end: String,
    #[serde(rename = "lineEnd")]
    line_end: String,
}
```

Field names exactly match comment-parser's `Tokens` type.
The `source` string is generated by concatenating all tokens fields
(same reconstruction rules as comment-parser's `stringify()`).

### Step 6.3: Attach source Array to JsdocTag

In comment-parser, `tag.source` holds a subset of lines belonging to that tag.
The serializer also reconstructs this:

```rust
fn build_tag_source_array(tag: &JsdocTag<'_>, start_line: u32) -> Vec<SerSourceLine> {
    // First tag line + type_lines (continuation) + description_lines
}
```

### Step 6.4: Integration with compat_mode Output

Add `source` field to SerBlock/SerTag when `compat_mode: true`:

```json
{
  "type": "JsdocBlock",
  "source": [
    { "number": 0, "source": "/**", "tokens": { ... } },
    { "number": 1, "source": " * Description", "tokens": { ... } },
    ...
  ],
  "descriptionLines": [...],
  "tags": [
    {
      "type": "JsdocTag",
      "source": [
        { "number": 2, "source": " * @param {string} name - desc", "tokens": { ... } }
      ],
      ...
    }
  ]
}
```

### Impact on Binary AST

**Do not include source array in Binary AST (recommended).**

The source array can be reconstructed from arena AST fields, so there is no need
to complicate the Binary AST format.
Share the reconstruction logic as a JS utility and call it from the Binary AST decoder:

```
Binary AST -> Decoder -> Lazy Nodes -> reconstructSourceArray(node) -> source[]
```

Since the JSON serializer and Binary AST decoder can use the same reconstruction logic,
extract `buildSourceArray(block)` as a JS utility in a shared package.

## Verification

### Existing Tests and Benchmark Regression

1. Verify all tests pass with `cargo test`
2. Verify no performance regression with `cargo bench --bench parser` and
   `cargo bench --bench serializer`
3. Verify no JS binding regression with `pnpm benchmark:ox-jsdoc`

### jsdoccomment Compatibility Tests

Based on jsdoccomment's test suite (`refers/jsdoccomment/test/`),
verify that ox-jsdoc's `compat_mode: true, empty_string_for_null: true` output
matches jsdoccomment's output.

#### Test Data Sources

- `refers/jsdoccomment/test/commentParserToESTree.test.js` -- 16 tests,
  inline expected AST structure. Verifies all fields of JsdocBlock/JsdocTag/
  JsdocDescriptionLine/JsdocTypeLine/JsdocInlineTag
- `refers/jsdoccomment/test/parseComment.test.js` -- 19 tests,
  including 4 inline tag formats (plain, pipe, space, prefix)
- `refers/jsdoccomment/test/fixture/roundTripData.js` -- 32 comment blocks,
  round-trip (parse -> AST -> string reconstruction) verification

#### Test Strategy

**Level 1: Field Existence Tests (Rust unit tests)**

Add Rust integration tests in `crates/ox_jsdoc/tests/compat/`.
Verify that expected fields exist in compat_mode JSON output
and ox-jsdoc-specific fields are excluded.

Target checklist:

JsdocBlock fields:

- `delimiter`, `postDelimiter`, `initial`, `terminal`, `lineEnd` are output
- `delimiterLineBreak`, `preterminalLineBreak` are output
- `endLine`, `descriptionStartLine`, `descriptionEndLine` are output
- `lastDescriptionLine`, `hasPreterminalDescription` are output
- `hasPreterminalTagDescription` is output only when applicable
- `start`, `end`, `range` follow `include_positions` setting

JsdocTag fields:

- `delimiter`, `postDelimiter`, `postTag`, `postType`, `postName` are output
- `initial`, `lineEnd` are output
- `optional`, `defaultValue`, `rawBody`, `body` are excluded

JsdocInlineTag fields:

- `rawBody` is excluded
- `format: "unknown"` is mapped to `"plain"`

null vs empty string:

- `rawType`, `name`, `namepathOrURL`, `text` are output as `""` when absent

**Level 2: AST Value Match Tests (JS integration -- dynamic comparison with jsdoccomment's actual parser)**

Call jsdoccomment's actual parser at test execution time and compare dynamically with ox-jsdoc's output.
No dependency on static JSON expected-value files.

Test runner: Node.js test script (vitest or node:test)
Dependencies: `@es-joy/jsdoccomment` (devDependencies), `ox-jsdoc` (workspace)

Test procedure:

1. Load fixture comment strings
2. **Parse with jsdoccomment** to get ESTree AST (A)
3. **Parse with ox-jsdoc using `compat_mode: true, empty_string_for_null: true`** to get AST (B)
4. Compare A and B field by field

```javascript
import { parseComment, commentParserToESTree } from '@es-joy/jsdoccomment'
import { parse } from 'ox-jsdoc'

for (const fixture of fixtures) {
  // Parse with jsdoccomment -> ESTree conversion
  const parsed = parseComment({ value: fixture.body })
  const expected = commentParserToESTree(parsed, 'jsdoc')

  // Parse with ox-jsdoc in compat_mode
  const actual = parse(fixture.source, { compatMode: true, emptyStringForNull: true })

  // Compare field by field (after stripping excluded fields)
  assertCompatible(actual.ast, expected)
}
```

Comparison target fields (all nodes):

- `type` -- node type name matches
- `delimiter`, `postDelimiter`, `initial` -- whitespace-preserving values match
- `description` -- text content matches

JsdocBlock additional:

- `terminal`, `lineEnd`, `delimiterLineBreak`, `preterminalLineBreak`
- `endLine`, `descriptionStartLine`, `descriptionEndLine`
- `lastDescriptionLine`, `hasPreterminalDescription`
- `descriptionLines[]` element count and each element's field values
- `tags[]` element count
- `inlineTags[]` element count

JsdocTag additional:

- `tag`, `rawType`, `name`, `postTag`, `postType`, `postName`
- `description`, `lineEnd`
- `typeLines[]` element count and each element's field values
- `descriptionLines[]` element count and each element's field values
- `inlineTags[]` element count

JsdocInlineTag additional:

- `tag`, `format`, `namepathOrURL`, `text`

source/tokens array (after Phase 6 completion):

- `source[]` element count matches
- Each `source[i].number` matches (0-based line number)
- Each `source[i].source` matches (original text reconstruction)
- Each `source[i].tokens` -- all 12 fields match
  (start, delimiter, postDelimiter, tag, postTag, type, postType,
  name, postName, description, end, lineEnd)
- `tag.source[]` element count and each entry matches

Comparison exclusions (stripped in assertCompatible):

- `start`, `end`, `range` -- ox-jsdoc-specific ESTree position info. Not present in jsdoccomment
- `parsedType` -- both null (ox-jsdoc not implemented, jsdoccomment without type parser)
- ox-jsdoc-specific fields -- already excluded in compat_mode

Since tests depend on jsdoccomment's actual parser, differences are automatically detected
on jsdoccomment version upgrades. No static JSON expected-value maintenance required.

**Level 3: Round-Trip Tests (comparison with jsdoccomment's estreeToString)**

Using the 32 comment blocks from `refers/jsdoccomment/test/fixture/roundTripData.js`:

1. Parse with jsdoccomment -> reconstruct string with `estreeToString()` -> reconstructed string (A)
2. Parse with ox-jsdoc in `compat_mode: true` -> reconstruct from source/tokens array -> reconstructed string (B)
3. Compare A and B

```javascript
import { parseComment, commentParserToESTree, estreeToString } from '@es-joy/jsdoccomment'
import { stringify } from 'comment-parser'
import { parse } from 'ox-jsdoc'
import { roundTripData } from './refers/jsdoccomment/test/fixture/roundTripData.js'

for (const { comment } of roundTripData) {
  // jsdoccomment round-trip
  const parsed = parseComment({ value: comment })
  const jdcAst = commentParserToESTree(parsed, 'jsdoc')
  const jdcRestored = estreeToString(jdcAst)

  // ox-jsdoc compat_mode output includes source[], so
  // it can be passed directly to comment-parser's stringify()
  const oxAst = parse(`/**${comment}*/`, { compatMode: true }).ast
  const oxRestored = stringify(oxAst) // use comment-parser's stringify directly

  // Does it match jsdoccomment's reconstructed result?
  assert.strictEqual(oxRestored, jdcRestored)
}
```

After Phase 6 completion, ox-jsdoc's compat_mode output includes the `source[]` array,
so it can be passed directly to comment-parser's `stringify()` to reconstruct the comment string.
No need to implement stringify functionality in ox-jsdoc itself.

**Level 4: Inline Tag Format Tests**

Create test cases for all 4 formats:

```javascript
// plain: {@link Something}
// pipe:  {@link Something|display text}
// space: {@link Something display text}
// prefix: [display text]{@link Something}
```

For each format:

- Is the `format` field the correct value?
- Is `namepathOrURL` extracted correctly?
- Is `text` extracted correctly? (empty string for plain format)

#### Test Fixture Management

Fixtures are created within the ox-jsdoc repository.
**No static JSON expected-value files are created** -- expected values are
dynamically generated from jsdoccomment's actual parser at test execution time.

```
tests/
  compat/
    fixtures/
      single-line.jsdoc        # single-line comments
      multi-line.jsdoc         # multi-line comments
      with-tags.jsdoc          # comments with tags
      inline-tags.jsdoc        # inline tags in 4 formats
      multiline-type.jsdoc     # multi-line type expressions
      description-only.jsdoc   # description only
      no-description.jsdoc     # no description
      preterminal.jsdoc        # description on the */ line
    compat.test.mjs            # JS tests (dynamic comparison with jsdoccomment parser)
    helpers.mjs                # assertCompatible, reconstructFromAst etc
```

- **Level 1 (Rust tests)**: Load fixtures, verify field existence/exclusion in compat_mode output
- **Level 2-4 (JS tests)**: Use same fixtures, **run both** jsdoccomment and ox-jsdoc, dynamically compare output
- Differences are automatically detected on jsdoccomment version upgrades
- No static JSON expected-value maintenance

#### Known Acceptable Differences

The following are accepted as differences from jsdoccomment (excluded in tests):

1. `start`, `end`, `range` -- ox-jsdoc-specific position info (additive)
2. `parsedType` -- both null (type parser is out of scope)
3. Absolute position values -- base_offset handling may differ
4. Whitespace normalization -- minor differences in CR/LF/CRLF handling possible

## Implementation Notes on Differences from jsdoccomment

After thorough review of jsdoccomment's implementation code, the following subtle behavioral
differences affect compat_mode compatibility. Each must be considered during Phase implementation.

### Note 1: `lineEnd` Exists on JsdocTag ESTree Node

jsdoccomment's `commentParserToESTree()` destructures comment-parser's tokens,
excluding `end`, `delimiter`, `postDelimiter`, `start`, and spreads the remaining
as `...tkns` onto JsdocTag (line 333). Since comment-parser's tokens include
the `lineEnd` field, JsdocTag also **includes** `lineEnd`.

```javascript
// commentParserToESTree.js 327-334
const {
  end: ed,          // excluded
  delimiter: de,    // excluded
  postDelimiter: pd, // excluded
  start: init,      // excluded
  ...tkns           // <- lineEnd is included here
} = tokens;

const tagObj = { ...tkns, ... };  // <- lineEnd ends up on JsdocTag
```

**Resolution**: Add `line_end` to `JsdocTag` in Phase 1.3, and output it as
`lineEnd` on the JsdocTag ESTree node when compat_mode is enabled.

### Note 2: `rawType` Has Encapsulating `{}` Stripped

jsdoccomment's `rawType` has the leading `{` and trailing `}` removed via
`stripEncapsulatingBrackets()`. For multi-line types, removal is from the
first line's beginning and the last line's end.

**Resolution**: When compat_mode is enabled, output `rawType` with `{}` stripped,
matching jsdoccomment. ox-jsdoc's `JsdocTypeSource.raw` should already have braces
stripped, but verify during implementation.

- `compat_mode: false` (default): maintain current ox-jsdoc `rawType` output
- `compat_mode: true`: output `rawType` with `{}` stripped, matching jsdoccomment

### Note 3: First JsdocTypeLine/JsdocDescriptionLine Has Empty String Fields

In jsdoccomment, the **first** JsdocTypeLine and JsdocDescriptionLine within a tag have
`delimiter`, `postDelimiter`, `initial` all set to `""` (empty string).
Lines from the second onward have actual margin values (`"*"`, `" "`, `" "` etc.).

This reflects comment-parser's source structure where the first line of a tag shares
the same line as the tag name and thus has no independent delimiter.

```javascript
// Example: @param {string} name - desc
typeLines[0] = { delimiter: '', postDelimiter: '', initial: '', rawType: 'string' }
// Example: second line of multi-line type
typeLines[1] = { delimiter: '*', postDelimiter: ' ', initial: ' ', rawType: 'number}' }
```

Similarly, description text on the same line as `/**` also has
`delimiter: ""`, `postDelimiter: ""`.

**Resolution**: In Phase 1.2 context.rs, set delimiter/postDelimiter/initial to empty strings
for TypeLines/DescriptionLines generated from a tag's first line.
Phase 6 source reconstruction also follows this rule.

### Note 4: Tag-Specific Parse Rules (`noTypes`/`noNames`/`@template`)

jsdoccomment's `parseComment()` customizes comment-parser's tokenizers,
skipping type or name parsing for specific tags:

**`defaultNoTypes`** -- tags that skip type parsing:
`@default`, `@defaultvalue`, `@description`, `@example`, `@file`,
`@fileoverview`, `@license`, `@overview`, `@see`, `@summary` etc.

**`defaultNoNames`** -- tags that skip name parsing:
`@returns`, `@return`, `@throws`, `@exception`, `@access`,
`@lends`, `@class`, `@constructor` etc.

**`@template` special handling** -- custom parsing of comma-separated template parameters.
Also supports bracketed default values `[T=default]`.

**`@see` `{@link}` special handling** (`hasSeeWithLink`) -- when `@see` tag's value
starts with `{@link ...}`, name parsing is skipped.

**Resolution**: ox-jsdoc's parser needs to implement these tag-specific rules.
If the current parser performs generic TagBody extraction, add logic to switch
type/name extraction on/off based on tag name.
This may require parser improvements as a prerequisite for Phase 1.

### Note 5: description Joining Logic and Empty Lines in descriptionLines

#### Three-Way Comparison

| Behavior                 | ox-jsdoc (current) | oxc_jsdoc                                                          | jsdoccomment                                                                        |
| ------------------------ | ------------------ | ------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| Empty line handling      | excluded           | `parsed()`: excluded / `parsed_preserving_whitespace()`: preserved | compact: excluded for both block and tag descriptions / preserve: preserved         |
| descriptionLines empties | **excluded**       | no descriptionLines concept                                        | compact: **block excluded, tag trailing empties excluded** / preserve: **included** |
| Spacing mode             | none               | method-based (2 variants)                                          | option (compact/preserve)                                                           |
| tagDescriptionSeen       | none               | none                                                               | present (ignored until type closes)                                                 |
| Leading/trailing empties | dropped            | dropped (parsed)                                                   | compact: excluded / preserve: included                                              |

#### Detailed descriptionLines Behavior by jsdoccomment Spacing

After thorough review of the conditional branch in `commentParserToESTree.js` (line 429):

```javascript
// Outer condition: determines whether to add a descriptionLine
if (((spacing === 'compact' && description) || lastTag) ||
    (spacing === 'preserve' && (description || delimiter !== '/**'))) {
```

**compact mode:**

- Block description: `description` is empty string (falsy) -> outer condition is false -> **descriptionLine not generated**
- Tag description (lastTag truthy): outer condition is true -> inner conditions apply:
  - First descriptionLine -> **always added** (even if empty. `isFirstDescriptionLine` special case)
  - Subsequent empty descriptionLines -> **not added** (lines 456-457: `spacing === 'compact' && (description || isFirstDescriptionLine)`)

**preserve mode:**

- `description` is non-empty, or `delimiter !== '/**'` -> **all descriptionLines are added**

**`description` joined text (compact mode):**

- Block description: empty lines already excluded by outer condition, not included in joining
- Tag description: additional condition (lines 482-483):
  `!(spacing === 'compact' && holder.description && description === '')`
  -> When description text already exists, joining of empty descriptions is skipped

#### Issues

1. **Empty lines excluded from descriptionLines**: ox-jsdoc's `parse_description_lines()`
   skips empty lines with `line.content.trim().is_empty()`. In jsdoccomment's preserve mode,
   empty lines are preserved as `JsdocDescriptionLine { description: "" }`.
   compat_mode + preserve must match this behavior.

2. **Missing `tagDescriptionSeen` flag**: In jsdoccomment, for multi-line types,
   tag description lines are ignored until the type closes with `}`. Neither ox-jsdoc
   nor oxc_jsdoc has this control. compat_mode output will be inconsistent for multi-line types.
   Condition: `tagDescriptionSeen ||= Boolean(lastTag && (rawType === '' || rawType?.endsWith('}')))`

3. **Joined text differences**: ox-jsdoc's `normalize_lines()` drops leading/trailing empty lines,
   but jsdoccomment's preserve mode includes them. The `description` field values may differ.

#### Resolution

**Phase 1.2 (context.rs) modifications:**

Modify `parse_description_lines()` to include empty lines in descriptionLines instead of excluding them:

```rust
// Before: skip empty lines
for line in lines {
    if line.content.trim().is_empty() {
        continue;  // <- remove this
    }
    description_lines.push(JsdocDescriptionLine { ... });
}

// After: preserve empty lines as descriptionLines
for line in lines {
    description_lines.push(JsdocDescriptionLine {
        span: Span::new(line.content_start, line.content_end),
        delimiter: ...,
        post_delimiter: ...,
        initial: ...,
        description: if line.content.trim().is_empty() { "" } else { line.content.trim_end() },
    });
}
```

This change applies regardless of compat_mode (internal Rust AST change).
Including empty lines has no impact on oxlint lint rules (empty descriptionLines
are simply additional entries with description: `""`).

**Compact mode filtering in the serializer layer:**

The Rust AST always preserves empty lines, but in compat_mode + compact,
the serializer filters empty lines to match jsdoccomment's behavior:

- **Block description descriptionLines**: exclude entries where description is empty string
- **Tag description descriptionLines**: always output the first descriptionLine, exclude subsequent empty lines
- **description joined text**: skip empty lines during joining (both block and tag)

In compat_mode + preserve: no filtering (all entries are output).

When compat_mode is disabled: maintain current ox-jsdoc behavior.

**`description` joined text:**

Maintain `normalize_lines()`'s leading/trailing empty line dropping.
This behavior approximately matches jsdoccomment's compact mode joining result.

**`tagDescriptionSeen` flag:**

Add logic to the parser to skip description lines until the type closes with `}`
for tags with multi-line types.
Implement as part of Phase 0 (parser modifications), alongside Note 4's tag-specific parse rules.

**`spacing` mode support:**

Add `spacing` field to SerializeOptions (handled in Note 7).
compat_mode default is compact. In preserve mode, preserve empty lines as `'\n'`
in description joining.

### Note 6: Single Empty descriptionLine Removal

When a tag has exactly one description line and its description is an empty string,
jsdoccomment truncates the `descriptionLines` array to length 0.

**Resolution**: Implement this filtering in Phase 6's source reconstruction
or in the serializer.

### Note 7: `spacing` Option

jsdoccomment's `commentParserToESTree()` accepts a `spacing` option
(`'compact'` or `'preserve'`, default `'compact'`).
In compact mode, empty description lines are filtered and description joining changes.

**Resolution**: When compat_mode is enabled, match jsdoccomment's behavior.

- `compat_mode: true` + `spacing` unspecified -> **compact** (matches jsdoccomment's default)
- `compat_mode: true` + `spacing: preserve` -> preserve mode can be specified
- `compat_mode: false` -> `spacing` has no effect (maintain current ox-jsdoc behavior)

```rust
pub enum SpacingMode {
    Compact,  // jsdoccomment's default
    Preserve,
}

pub struct SerializeOptions {
    pub compat_mode: bool,
    pub empty_string_for_null: bool,
    pub include_positions: bool,
    pub spacing: SpacingMode, // only effective when compat_mode. Default: Compact
}
```

Scope of spacing's effect (when compat_mode):

| Target                          | compact                                                    | preserve                       |
| ------------------------------- | ---------------------------------------------------------- | ------------------------------ |
| Block `descriptionLines[]`      | **Exclude** entries with empty description                 | Include all entries            |
| Tag `descriptionLines[]`        | Always include first entry, **exclude** subsequent empties | Include all entries            |
| Block `description` joined text | Skip empty lines during joining                            | Preserve empty lines as `'\n'` |
| Tag `description` joined text   | Skip empty description joining when text already exists    | Preserve empty lines as `'\n'` |
| `typeLines[]` rawType joining   | Trim and join                                              | Preserve whitespace            |

See Note 5 for detailed conditional branching.

### Note 8: Optional Field JSON Output

In jsdoccomment, `undefined` (unset) fields are not output in JSON.
Optional fields such as `hasPreterminalTagDescription`, `descriptionStartLine`,
`descriptionEndLine`, `lastDescriptionLine` must **omit the field entirely**
when the value is absent, rather than outputting `null`.

**Resolution**: Use serde's `#[serde(skip_serializing_if = "Option::is_none")]`.
Apply explicitly in Phase 2.3.

### Note 9: source Reconstruction When Opening/Closing Lines Have Content

#### Three-Way Comparison

| Case             | comment-parser                                                | oxc_jsdoc          | ox-jsdoc                                |
| ---------------- | ------------------------------------------------------------- | ------------------ | --------------------------------------- |
| `/**` handling   | Preserved as `tokens.delimiter="/**"`                         | Stripped by caller | `body_range()` skips 3 bytes            |
| `*/` handling    | Coexists on same line as `tokens.end="*/"`                    | Stripped by caller | `body_range()` removes trailing 2 bytes |
| `*` on `*/` line | **Not** consumed as delimiter (`!rest.startsWith(end)` guard) | N/A                | May be consumed as margin `*`           |

#### Problematic Cases

**Single-line: `/** @type {string} \*/`\*\*

comment-parser's source[0]:

```javascript
{ delimiter: "/**", postDelimiter: " ", description: "@type {string} ", end: "*/" }
// -> spec-parser decomposes description into tag/type/name/description
```

**Tag on `*/` line: `/** \n _ @param {string} name _/`\*\*

comment-parser's final source entry:

```javascript
{ delimiter: "*", postDelimiter: " ", description: "@param {string} name ", end: "*/" }
// delimiter="*" and end="*/" coexist on the same line
```

**`*/` only line: ` */`**

comment-parser's final source entry:

```javascript
{ delimiter: "", postDelimiter: "", description: "", end: "*/" }
// `*` is not consumed as delimiter (`*/` guard)
```

#### Resolution

When compat_mode is enabled, reproduce the same behavior as comment-parser's source structure.
Implement Phase 6's source reconstruction logic following comment-parser's rules:

**comment-parser rules:**

1. `/**` always goes into source[0]'s `tokens.delimiter`
2. If content exists on the same line as `/**`, it goes into source[0]'s `tokens.description`
3. `*/` goes into `tokens.end` of the line it appears on (not a separate line)
4. `*` on the `*/` line is **not** consumed as `tokens.delimiter`
   (`!rest.startsWith(markers.end)` guard)
5. If `/**` and `*/` are on the same line, source array is 1 entry:
   `{ delimiter: "/**", ..., description: "content", ..., end: "*/" }`

**Phase 6 implementation:**

```
source reconstruction decision logic:

1. delimiter_line_break == "" (content on opening line):
   -> Merge into source[0] with delimiter="/**" + content + (end="*/" if single-line)
   -> Do not generate separate description_line/tag_line

2. preterminal_line_break == "" (content on closing line):
   -> Set tokens.end="*/" on the last content line
   -> Do not generate separate closing line

3. delimiter_line_break != "" and preterminal_line_break != "" (normal):
   -> source[0] = opening line (delimiter="/**")
   -> source[last] = closing line (delimiter="", end="*/")

4. */ only line:
   -> delimiter="" (do not consume * as delimiter, per comment-parser's guard)
   -> end="*/"
```

Use `delimiter_line_break` and `preterminal_line_break` values added in Phase 1
to determine the case.

### Note 10: `@` Prefix Removal

jsdoccomment removes `@` from the `tag` field (`tag.replace(/^@/v, '')`).
comment-parser's source[].tokens.tag stores with `@` prefix (`"@param"`).

**Resolution**: When compat_mode is enabled, match comment-parser's behavior:

- ESTree AST `JsdocTag.tag` field -> without `@` (`"param"`)
  -- jsdoccomment's `commentParserToESTree()` removes it
- source[].tokens.tag -> with `@` (`"@param"`)
  -- comment-parser's source structure as-is

Verify at implementation time whether ox-jsdoc's Rust AST (`JsdocTagName.value`)
already stores without `@`. If so:

- compat_mode ESTree output: output as-is
- Phase 6 source reconstruction: re-prepend `@` to tokens.tag to produce `"@param"`

### Note 11: Handling `name` on a Separate Line

jsdoccomment's `commentParserToESTree()` (lines 337-356) scans subsequent source entries
when the tag's `name` token is not on the tag line:

```javascript
if (!tokens.name) {
  let i = 1
  while (source[idx + i]) {
    const {
      tokens: { name, postName, postType, tag: tg }
    } = source[idx + i]
    if (tg) break // stop when next tag is reached
    if (name) {
      tkns.postType = postType
      tkns.name = name
      tkns.postName = postName
      break
    }
    i++
  }
}
```

In this case, JsdocTag's `postTag` and `postType` become `''` (empty string),
and `estreeToString()` detects this condition to insert a newline
(functioning as a marker indicating that name is on the next line).

**Resolution**: In ox-jsdoc's parser, names are normally extracted from the same line
as the tag. Cases where this spans multiple lines include:

- Multi-line types where name is on the line after the closing `}`
- Tag name immediately followed by a newline, with name on the next line

In compat_mode, the serializer must output `postTag: ""` and `postType: ""`
when name is on a separate line. Phase 6's source reconstruction must also
account for this case.

## Test Strategy Amendments

### Amendment 1: Level 2 Test API Call

`parseComment()`'s signature is `(commentOrNode, indent)` with 2 arguments.
Use `commentParserToESTree()` for ESTree conversion:

```javascript
// Before (incorrect):
const expected = parseComment({ value: fixture.body }, 'jsdoc', { mode: 'jsdoc' })

// After:
import { parseComment, commentParserToESTree } from '@es-joy/jsdoccomment'
const parsed = parseComment({ value: fixture.body })
const expected = commentParserToESTree(parsed, 'jsdoc')
```

### Amendment 2: parsedType Comparison Exclusion

jsdoccomment stores `jsdoc-type-pratt-parser`'s AST in `parsedType` when a valid type
is present (not null). ox-jsdoc always outputs null.

**Resolution**: **Strip `parsedType` from both** in assertCompatible().
Even when ox-jsdoc is null and jsdoccomment is non-null, exclude from comparison.

### Amendment 3: Add `spacing` Mode Tests

Add preserve mode tests in addition to compact mode (default).
Include spacing-specific test cases in Level 2 tests:

```javascript
for (const spacing of ['compact', 'preserve']) {
  const parsed = parseComment({ value: fixture.body })
  const expected = commentParserToESTree(parsed, 'jsdoc', { spacing })
  const actual = parse(fixture.source, { compatMode: true, spacing })
  assertCompatible(actual.ast, expected)
}
```

### Amendment 4: Add Tag-Specific Parse Rule Tests

Add fixtures to verify `noTypes`/`noNames` tag compatibility:

```
tests/compat/fixtures/
  no-types-tags.jsdoc    # @example, @description, @see etc
  no-names-tags.jsdoc    # @returns, @throws, @access etc
  template-tag.jsdoc     # @template T, U, [V=default]
  see-with-link.jsdoc    # @see {@link Foo}
```

## Implementation Order

```
Phase 1.1 -> 1.2 -> 1.3 -> 1.4 (Critical path: source-preserving fields)
                                |
                           Phase 2 (line metadata)
                                |
                           Phase 3 (null vs "")
                                |
                      Phase 4 + 5 (visitor keys + format adjustment)
                                |
                           Phase 6 (source/tokens array reconstruction)
```

Phase 1 has the largest blast radius, and Phases 2-6 depend on Phase 1.4's SerializeOptions.
Phase 6 can only build a complete source array once all fields from Phases 1-2 are available,
so it is implemented last.

Phase 6 is directly tied to eslint-plugin-jsdoc rule compatibility, so Phase 6 is required
if integration with eslint-plugin-jsdoc is the goal.
For jsdoccomment AST-level compatibility only, Phases 1-5 are sufficient.

Note 4 (tag-specific parse rules) may require parser improvements as a prerequisite for Phase 1.
noTypes/noNames/template special handling is a parser-layer concern that cannot be addressed
by the serializer alone. Before starting implementation, verify ox-jsdoc parser's current behavior
and if necessary, prioritize parser modifications as Phase 0.
