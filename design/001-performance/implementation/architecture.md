# Near-Term Recommended Architecture

The most natural implementation split is:

1. **Scanner / parser**
   Input: comment text
   Output: `JSDocComment<'a>`

2. **Validator**
   Input: AST + mode / tag dictionary
   Output: diagnostics + normalized semantic facts

3. **Analyzer**
   Input: AST + validated facts
   Output: higher-level data for linters, formatters, and other consumers

4. **Serializer**
   Input: AST
   Output: JSON payload for JS consumers

The core idea borrowed from `oxc` is not a specific implementation detail.
It is the separation of parser concerns from consumer-specific concerns.

## Next Implementation Steps

The performance-sensitive design is now sufficient to start the initial parser
implementation. The next work should focus on building the system in the same
order as the architecture above.

### Step 1. Parser skeleton

Implement the first parser crate / module skeleton around:

- `parse_comment(...) -> ParseOutput<'a>`
- `ParserContext<'a>`
- `Checkpoint`
- internal scanner helper methods
- parser diagnostics helpers

This step should implement the public parser entrypoint and enough internal
state to parse a complete JSDoc block into `JSDocComment<'a>`.
It should not expose a public token / event stream.

### Step 2. Minimal parser behavior

Implement the smallest parser behavior that exercises the performance contract:

- block comment boundary recognition
- line-prefix stripping
- top-level `Description`
- block tags with `GenericTagBody`
- inline tags in descriptions
- `raw_body` preservation
- borrowed-slice-first string fields
- absolute `Span` calculation with `base_offset`
- minimal v1 diagnostics

This step is where direct AST construction with internal checkpoints should be
introduced. Checkpoints should be used only around ambiguous regions such as
inline tags, type expressions, optional parameter syntax, and fenced code.

### Step 3. Fixture and benchmark seed

Create the initial fixture layout before growing parser behavior too far:

- `fixtures/perf/common`
- `fixtures/perf/description-heavy`
- `fixtures/perf/type-heavy`
- `fixtures/perf/special-tag`
- `fixtures/perf/malformed`
- `fixtures/perf/toolchain`

Use `.jsdoc` files for exact parser input and sidecar `.json` files for
metadata and expectations.

Initial fixtures should cover:

- a common `@param` / `@returns` comment
- description with `{@link}` or `{@linkcode}`
- a long or TypeScript-like type expression
- a special tag such as `@typeParam` or `@borrows`
- a recoverable malformed inline tag
- a custom-tag comment from a toolchain-oriented source

### Step 4. Validator stub

After the parser returns stable AST shapes, add a validator stub that can consume
the AST without changing parser behavior.

The validator should start with:

- built-in tag lookup scaffolding
- mode placeholder
- parser-independent diagnostic emission
- no eager normalization in the parser

The goal is not to finish semantic validation immediately.
The goal is to prove that parser / validator separation works in code.

### Step 5. Analyzer and serializer stubs

Add analyzer and serializer stubs after the parser and validator boundary is
usable.

The analyzer should initially expose only simple consumer facts.
The serializer should initially target the JSON shape, not raw transfer.

This keeps the near-term architecture connected:

```text
Scanner / parser -> Validator -> Analyzer -> Serializer
```

### Step 6. Measurement before optimization

Do not add deeper lexer micro-optimizations, string interning, public event
streams, or raw-transfer-oriented layout constraints before the initial parser
benchmarks exist.

The first measurements should compare:

- scanner / parser only
- parser + validator stub
- parser + validator + serializer stub

Only after these exist should the implementation consider specialized byte
search, pre-hashed tag names, or more aggressive layout tuning.
