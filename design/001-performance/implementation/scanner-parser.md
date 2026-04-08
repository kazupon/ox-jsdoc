# Scanner / Parser Boundary

The scanner / parser boundary should be decided before the first parser
implementation, because it affects allocation behavior, recovery, diagnostics,
and future API stability.

## Option A. Full token stream before parsing

Shape:

```text
source_text -> Scanner -> Vec<Token> -> Parser -> AST
```

Pros:

- simple mental model
- parser can be written against a stable token sequence
- diagnostics can point to token spans easily
- token stream can be reused by tests or debugging tools

Cons:

- extra allocation for `Vec<Token>`
- duplicate storage of spans and token payload metadata
- harder to keep the common path allocation-light
- token kinds become another design surface that can drift from the AST
- not obviously necessary for JSDoc, where the parser is much smaller than a JS / TS parser

Decision:

- do not use this as the v1 architecture
- keep it as a possible internal optimization only if measurement or recovery complexity justifies it

## Option B. Public token / event stream API

Shape:

```rust
pub fn scan_comment(source_text: &str) -> Vec<Token>;

pub fn events(source_text: &str) -> impl Iterator<Item = Event>;
```

Pros:

- useful for external tooling that wants lower-level comment structure
- useful for debugging parser behavior
- can support consumers that do not need the full AST

Cons:

- exposes a second public compatibility surface besides the AST
- makes internal parser changes harder
- requires token / event stability before the AST implementation is proven
- can force the parser to preserve intermediate details that are not useful to normal consumers

Decision:

- do not expose a public token / event stream in v1
- revisit only after the AST, validator, and serializer APIs have stabilized

## Option C. Direct AST construction without checkpoints

Shape:

```text
source_text -> Parser cursor -> AST
```

Pros:

- simplest and lightest architecture
- no token-stream allocation
- easy to preserve borrowed slices
- shortest common path for well-formed comments

Cons:

- fragile recovery for malformed inline tags, type expressions, and fences
- difficult to try an interpretation and fall back to text
- failures can force either eager diagnostics or overly broad text fallback
- parser code can become tangled if ambiguous regions are handled ad hoc

Decision:

- too weak for real-world JSDoc recovery as the only strategy
- useful as the baseline, but needs limited checkpoints around ambiguous regions

## Option D. Direct AST construction with internal checkpoints

Shape:

```text
source_text
  -> Parser cursor
  -> small internal checkpoints around ambiguous regions
  -> AST allocation after each subpart is accepted
  -> ParseOutput { comment, diagnostics }
```

Pros:

- keeps the public API simple: `parse_comment(...) -> ParseOutput<'a>`
- avoids full token-stream allocation on the common path
- preserves borrowed-slice-first string handling
- supports recovery for ambiguous regions
- keeps token / event details private
- gives the implementation room to change scanner internals later

Cons:

- parser implementation is more careful than a pure token-stream parser
- checkpoint discipline must be enforced
- ambiguous regions must avoid arena allocation until accepted
- debugging may initially be less convenient without a public token stream

Decision:

- use this as the v1 architecture

## Checkpoint contract

Checkpoints are internal parser state, not public API.
They should be small and should not own AST nodes.

It is useful to distinguish a long-lived parser context from a checkpoint:

- `ParserContext`
  - current parsing state for the whole `parse_comment` call
  - owns or references `source_text`, `base_offset`, current cursor state, diagnostics, and parser options
  - lives for the duration of parsing
- `Checkpoint`
  - a small rollback snapshot of selected parser / scanner state
  - used only around ambiguous regions
  - does not own diagnostics, AST nodes, the allocator, or source text

In other words, a checkpoint is not the parser context itself.
It is a copyable subset of the context that is safe to restore.

Representative shape:

```rust
struct ParserContext<'a> {
    source_text: &'a str,
    base_offset: u32,
    offset: u32,
    diagnostics: Vec<OxcDiagnostic>,
    options: ParseOptions,
    brace_depth: u16,
    bracket_depth: u16,
    paren_depth: u16,
    quote: Option<QuoteKind>,
    fence: Option<FenceState>,
}

struct Checkpoint {
    offset: u32,
    brace_depth: u16,
    bracket_depth: u16,
    paren_depth: u16,
    quote: Option<QuoteKind>,
    fence: Option<FenceState>,
}
```

Use checkpoints only around ambiguous regions:

- trying to parse `{@link ...}` as an inline tag
- trying to parse `{Object.<string, number>}` as a type expression
- scanning optional parameter syntax such as `[name=default]`
- detecting fenced code boundaries

Rules:

1. checkpoint state stores cursor and shallow scanner state only
2. checkpoint state does not store AST nodes
3. checkpoint state does not require arena rollback
4. ambiguous regions should compute local spans / slices first
5. arena allocation should happen only after the subpart is accepted

This rule is important because arena allocators are not a good fit for frequent
fine-grained rollback. If a parse attempt may fail, it should avoid committing
AST nodes until the interpretation is accepted.

## v1 boundary decision

The v1 boundary is:

```text
direct AST construction
+ internal scanner helpers
+ small rollback checkpoints
- public token / event stream
- full tokenization pass
- arena rollback
```

`Scanner` does not need to be a public type.
The parser can own the cursor and expose scanner-like helpers internally.

The implementation should stay free to introduce an internal token or event
stream later, but only if measurement shows that it improves recovery,
diagnostics, or throughput on representative inputs.
