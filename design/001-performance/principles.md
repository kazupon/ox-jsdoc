# Principles and Reference Points

## Goals

`ox-jsdoc` should remain fast enough for large codebases without becoming
unnecessarily rigid or transport-driven.

The goal is not to recreate `oxc` as-is.
The goal is to adapt the parts of its design that are genuinely useful for JSDoc parsing.

The main goals are:

1. Keep the parse hot path small.
2. Keep AST traversal cache-friendly.
3. Avoid importing transport-specific constraints into the core too early.
4. Preserve enough structure for stricter validation and richer analysis later.

## Assumptions Already Fixed

The following directions can be treated as fixed assumptions for now:

1. **Arena allocator**
   AST memory assumes an `oxc_allocator`-style arena model.

2. **JSON-first JS transfer**
   Transfer to JS starts with JSON, not raw transfer.

3. **Diagnostics based on the `oxc` model**
   Parser diagnostics should be designed in the same general direction as
   `oxc_diagnostics` and the `oxc` parser error model.

These choices preserve the most important performance characteristics while
avoiding premature commitment to the most specialized parts of `oxc`.

## What to Take as Reference from `oxc`

### 1. Separation Between Parser and Semantic Phases

This is the most important design principle.

One reason `oxc` stays fast is that it does not try to perform every expensive
correctness check during parsing.
Some syntax-adjacent checks, binding work, symbol resolution, and higher-level
validity checks are deferred to later phases.

`ox-jsdoc` should follow the same direction.

The parser should mainly be responsible for:

- recognizing comment structure
- building a typed AST
- preserving spans and raw syntax needed for later interpretation
- recovering when possible without destroying tree structure

What the parser should not try to fully decide:

- final tag-specific validity for every built-in or custom tag
- complete `jsdoc` / `Closure` / `TypeScript` mode resolution
- complete namepath normalization
- complete type compatibility rules

These should be pushed to later semantic validation and analysis phases.

`ox-jsdoc` should think in at least three layers:

| Layer         | Primary role                                 | What it does                                                                                      | What it does not do                                             |
| ------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| **parser**    | Build syntax trees                           | comment structure recognition, AST construction, spans, raw syntax retention, limited recovery    | final tag meaning, final mode decisions, strict validation      |
| **validator** | Apply validity rules and light normalization | built-in/custom tag rules, type/namepath/parameter-path checks, mode-sensitive rules, diagnostics | consumer-facing high-level derived data                         |
| **analyzer**  | Produce consumer-oriented facts              | data for linters, formatters, renderers, and other consumers                                      | rebuilding the parse tree, defining validation rules themselves |

#### Concrete examples by layer

##### Example 1. `@param {string} userId - The user ID`

- parser
  - creates `JsdocTag(@param)`
  - creates `JsdocTagBody::Generic { type_source, value, separator, description }`
  - assigns spans to each part
- validator
  - checks whether `value` is a valid parameter name for `@param`
  - checks whether the type expression is valid in the current mode
  - may refine a `JsdocTagValue::Raw` into a parameter-name-oriented interpretation
- analyzer
  - can expose consumer-friendly facts such as:
    - “this comment documents one parameter”
    - “the parameter name is `userId`”
    - “the description is `The user ID`”

##### Example 2. `@variation 3`

- parser
  - may stop at `JsdocTagBody::Generic.value = JsdocTagValue::Raw("3")`
  - preserves `raw_body = "3"`
- validator
  - interprets the value according to `@variation` rules
  - applies variation-specific checks
- analyzer
  - can expose a normalized variation value if needed

##### Example 3. `@memberof! Foo.`

- parser
  - accepts incomplete raw `JsdocNamepathSource`
  - preserves `raw_body`
- validator
  - interprets the `!` semantics
  - checks whether the trailing connector form is valid
- analyzer
  - can expose a consumer-friendly “forced memberof” interpretation

##### Example 4. `@borrows source as target`

- parser
  - creates `JsdocTagBody::Borrows` directly because the syntax is lexically special
- validator
  - checks that source and target are valid enough namepath-like values
- analyzer
  - can expose the relationship as a borrow edge

The important distinction is:

- the **validator** answers “is this valid?” and performs minimal normalization
- the **analyzer** answers “how should downstream consumers use this?”

Practical consequences:

- `JsdocTagBody::Generic` should remain intentionally broad
- `raw_body` should be preserved
- the parser should tolerate nodes that are syntactically valid but semantically unresolved

This matches the current AST design and should be treated as a hard rule.

### 2. Compact Layout-Oriented Node Design

Here, `layout` means how AST nodes are arranged in memory.

Why this matters for performance:

- smaller common-case nodes fit better in CPU cache
- traversal reads less memory
- following child nodes becomes easier to reason about

If every node stores too much unrelated data directly, parsing and traversal pay
for unnecessary memory traffic on every comment.

`oxc` cares not only about arenas, but also about keeping common nodes small.
`ox-jsdoc` should adopt the same principle.

Rules worth adopting:

- meaningful nodes should generally carry a `Span`
  - this preserves source location information
- branching nodes should avoid storing heavy payloads inline when possible
  - in Rust terms, `Box` can serve as an “edge to another node”
- large child collections should live outside the parent node
  - arena-backed `Vec` is the natural container for this
- syntactic similarity should not force semantic over-unification
  - merging semantically different cases often increases branching later

Examples already reflected in the current design:

- `JsdocDescriptionLine<'a>` and `JsdocInlineTag<'a>`
  - line-level text and inline-tag boundaries are exposed separately, so rules
    can traverse inline tags without flattening all description syntax
- `JsdocTagBody<'a>`
  - generic bodies and `@borrows` bodies are separated
- `JsdocTagValue<'a>`
  - parameter names, namepaths, identifiers, and raw tokens are not reduced to one vague string
- `JsdocTypeSource` and future `JsdocType`
  - raw type syntax and the internal type tree are separated

For example, if description data tried to inline every possible future text,
inline-code, fenced-code, and inline-tag case into one large node, then even a
plain description line would require traversing a larger shape than necessary.

The current design lets common cases stay small:

- plain description text uses `JsdocDescriptionLine`
- inline tags use `JsdocInlineTag`
- fenced code can remain scanner state or become a future `JsdocFencedCodeBlock`

The important goal is not byte-for-byte similarity with `oxc_ast`.
The real goal is to keep common-path nodes small and understandable.

### 3. Lossless-Enough Parsing

`oxc` preserves enough source information to support diagnostics, formatting,
and downstream analysis. `ox-jsdoc` should do the same at the level needed for JSDoc.

What should be preserved:

- accurate spans
- source-preserving description lines
- inline-tag boundaries
- generic tag-body structure
- raw tag body for parts whose final meaning is deferred

This is why `raw_body` and generic body extraction belong in the core design.
They are not only correctness features. They are also performance choices,
because they reduce the need for eager normalization inside the parser.

#### Example 1. Accurate spans

Input:

```js
/**
 * @param {string} userId
 */
```

Useful spans to preserve:

- the full span of `JsdocTag(@param)`
- the span of `{string}`
- the span of `userId`

This allows later phases to:

- report diagnostics only on the type fragment
- target fixes only at the name fragment
- return to the original source at sub-part granularity

#### Example 2. Structured descriptions

Input:

```js
/**
 * Find a user. See {@link UserService} for details.
 */
```

Useful representation:

```text
JsdocBlock.description = "Find a user. See {@link UserService} for details."
JsdocBlock.description_lines = [
  JsdocDescriptionLine("Find a user. See {@link UserService} for details.")
]
JsdocBlock.inline_tags = [
  JsdocInlineTag(tag = "link", namepath_or_url = "UserService")
]
```

If this were flattened into a single string:

- inline tags would need to be rediscovered later
- formatting would become harder
- the line-level syntax and inline-tag boundary would be lost

#### Example 3. Inline-tag boundaries

Input:

```js
/**
 * Use {@link Foo#bar} or {@linkcode Baz}.
 */
```

Useful facts to preserve:

- the first inline tag is `link`
- the second inline tag is `linkcode`
- each one has its own body node

This allows later phases to:

- validate `link` and `linkcode` differently
- render them differently
- report diagnostics on one broken inline tag without touching the other

#### Example 4. Generic tag-body structure

Input:

```js
/**
 * @param {string} userId - The user ID
 * @since 2.0.0
 */
```

Useful representation:

```text
@param.body = JsdocTagBody::Generic {
  type_source = JsdocTypeSource("string"),
  value = JsdocTagValue::Parameter("userId"),
  separator = Dash,
  description = "The user ID"
}

@since.body = JsdocTagBody::Generic {
  type_source = None,
  value = JsdocTagValue::Raw("2.0.0"),
  separator = None,
  description = None
}
```

The key point is that `@param` and `@since` should not collapse into one raw string
at parse time. A minimal generic structure is already valuable.

This helps because:

- common tags can be parsed cheaply
- semantic interpretation can still differ later
- parser performance does not depend on the full tag dictionary

#### Example 5. Why preserve `raw_body`

Input:

```js
/**
 * @variation 3
 * @memberof! Foo.
 * @borrows source as target
 */
```

What is difficult at parse time:

- whether `3` should be treated as text or as something more structured
- how to treat an incomplete namepath in `@memberof! Foo.`
- whether `source as target` should be immediately interpreted as a special syntax

Useful preservation:

```text
JsdocTag.raw_body = "3"
JsdocTag.raw_body = "Foo."
JsdocTag.raw_body = "source as target"
```

Preserving `raw_body` means:

- generic parsing does not need to fail
- tag-specific parsers can be applied later
- source fidelity remains better during recovery and diagnostics

So `raw_body` is not just a fallback. It is a deliberate escape hatch that keeps
the parser from doing expensive eager interpretation.

#### Example 6. Why this matters for performance

If parsing tried to fully resolve all of the following up front:

- tag dictionary lookups
- mode resolution
- tag-specific body shapes
- final classification of namepath vs parameter path vs text
- strict validation

then even a normal `@param` would incur extra branches and work.

The current design aims for:

1. cheap extraction of `type? + value? + description?`
2. preservation of `raw_body` for hard cases
3. deferred strict interpretation in later phases

That split keeps the parse hot path short for common JSDoc comments.

### 4. Borrowed Slice First, Normalized String Later

`ox-jsdoc` should treat source text as the primary storage for parsed strings.
The parser should preserve spans and borrowed slices whenever possible, and
create normalized strings only when a later phase actually needs them.

This follows the same broad performance direction as `oxc`: avoid copying text
on the common path, and allocate only for cases that require normalization.

The parser hot path should avoid creating owned strings for:

- description text
- block tag `raw_body`
- inline tag bodies
- raw type text
- parameter and name tokens

Those values should initially remain tied to the original source text through
`Span` and borrowed string slices.

Normalization should be delayed to validator, analyzer, formatter, or serializer
layers when the consumer needs one of these forms:

- unescaped text
- whitespace-normalized descriptions
- joined description strings
- default values with quote handling
- rendered link labels
- JS-facing owned strings for serialization

#### Why descriptions must remain structured

Real-world projects contain many inline tags in descriptions.
For example, API-documentation-heavy sources commonly use forms such as:

```js
/**
 * Create a command from {@linkcode Command | entry command}.
 */
```

Useful representation:

```text
JsdocBlock.description_lines = [
  JsdocDescriptionLine("Create a command from {@linkcode Command | entry command}.")
]
JsdocBlock.inline_tags = [
  JsdocInlineTag(tag = "linkcode", namepath_or_url = "Command", text = "entry command")
]
```

Flattening this into one string would make later phases rediscover the inline tag
boundary. Keeping line-level description syntax plus direct inline-tag nodes
during parsing is both more accurate and cheaper for consumers such as linters,
formatters, and renderers.

This matters in real-world corpora:

- `../gunshi` contains many `{@link}` and `{@linkcode}` references
- `../../oss/intlify/vue-i18n` contains many `{@link}` references and long `@remarks`
- `refers/eslint-plugin-jsdoc` contains lint-oriented inline-tag edge cases

The exact counts are benchmark inputs, not part of the AST contract.
The design consequence is stable: inline-tag boundaries should be preserved
instead of rebuilt from flattened strings.

#### Fence-aware scanning

Fenced code blocks should be tracked as scanner state.
Inside a fenced block, text that looks like a tag should usually remain text:

````js
/**
 * @example
 * ```ts
 * // This is code, not a JSDoc block tag:
 * @decorator()
 * ```
 */
````

The scanner therefore needs a cheap `fence` state for description and example
parsing. This is not a request for a full Markdown parser. It is a small state
machine that prevents expensive or incorrect rediscovery later.

#### Scanner state should be shallow

The scanner should be byte-oriented, but it should not be a naive
`split_whitespace` parser.

Important ASCII markers include:

```text
@ { } [ ] ( ) < > | = - " ' ` \n \r whitespace
```

Useful shallow state includes:

- `brace_depth`
- `bracket_depth`
- `paren_depth`
- quote state
- inline-tag state
- fence state
- line-start / after-star-prefix state

This is necessary because real comments contain forms such as:

- `[opts.maxLength=Infinity]`
- `[subject="world"]`
- `{import('typedoc').TypeDocOptions & { docsRoot?: string }}`
- `{@linkcode Command | entry command}`
- fenced `@example` blocks

The principle is not to fully parse every nested language immediately.
The principle is to avoid splitting strings at obviously wrong boundaries.

#### Allocation rule

The default rule should be:

1. keep the source slice if it can be represented as a slice
2. keep `raw_body` for unresolved or tag-specific interpretations
3. allocate into the arena only when a normalized string is required

For v1, tag names and frequently compared small tokens can remain borrowed
strings. If measurement shows tag lookup or name comparison is hot, an
`Ident`-like interned or pre-hashed representation can be considered later.

### 5. Use Generated Code for Structural Repetition

`oxc` does not maintain layout assertions, visitors, and transfer helpers entirely by hand.
`ox-jsdoc` should also generate code when the repetition is structural.

Here, “structural repetition” means code that must be updated in many places
whenever the AST changes, even though the updates follow a mostly mechanical pattern.

If nodes such as `JsdocBlock`, `JsdocTag`, `JsdocDescriptionLine`,
`JsdocInlineTag`, and future `JsdocType` nodes are maintained by hand across
multiple support layers, the risk of drift is high.

Typical drift-prone areas:

- visitor `walk_*` functions
- serializer `match` arms
- layout checks
- dispatch tables by node kind

Problems that tend to happen with hand-maintained code:

1. a new node is added but the visitor is not updated
2. a serializer silently misses one variant
3. field ordering changes without triggering a layout-related check
4. similar rules diverge because they were duplicated manually

Promising generation targets:

- visitor / walker code
- layout assertions for representative nodes
- part of serializer boilerplate

#### What should be generated

##### 1. Visitor / walker code

Examples:

- `walk_jsdoc_block`
- `walk_jsdoc_description_line`
- `walk_jsdoc_tag`
- `walk_jsdoc_inline_tag`
- `walk_jsdoc_type`

These mostly follow the same pattern: visit children in order.
If derived from AST definitions, they become much harder to forget or desynchronize.

##### 2. Layout assertions

Examples:

- `size_of::<JsdocTag>()`
- `size_of::<JsdocGenericTagBody>()`
- `size_of::<JsdocDescriptionLine>()`
- `offset_of!(JsdocTag, body)`

Even without immediate raw transfer, `ox-jsdoc` should detect regressions such as:

- representative nodes growing unexpectedly
- field ordering drifting

This is not about freezing ABI too early.
It is about protecting performance-sensitive assumptions.

##### 3. Serializer boilerplate

If JS transfer uses JSON, the AST still needs to be converted into a JS-facing shape.

Here, “shape” means the final object structure seen in JS, including:

- field names
- nesting
- variant encoding such as `type` vs `kind`
- array vs object representation

For example:

- `{ type: "Text", value: "foo" }`
- `{ kind: "Text", text: "foo" }`

These may represent similar data, but they are different shapes for serializers and consumers.

If the AST definition already knows:

- which fields each node exposes
- which discriminants each variant uses

then a good part of the serializer boilerplate can be generated instead of repeated by hand.

#### Why this matters for performance

This is not only about raw runtime speed.
At first, it matters even more as a way to keep the design from drifting.

Generated support code helps:

- preserve compact-layout assumptions
- reduce missed visitor / serializer updates
- make later optimized implementations easier to swap in

In other words, it protects performance-sensitive invariants as the AST evolves.

#### Recommended generation order for `ox-jsdoc`

There is no need to generate everything at once.
A practical order is:

1. visitor / walker
   - changes in the AST quickly require it
   - hand-maintained omissions are common

2. layout assertions for representative nodes
   - such as `JsdocTag`, `JsdocGenericTagBody`, `JsdocDescriptionLine`, and future `JsdocType`

3. helper code for JSON serialization
   - once the transfer format starts to stabilize

This captures the useful part of the `oxc` approach without importing unnecessary complexity too early.

### 6. Separate Fast Paths from Cold Paths

`oxc` consistently keeps common cases short and pushes rare cases to the back.
`ox-jsdoc` should do the same.

Definitions:

- **fast path**: the short, simple path used for common input
- **cold path**: the heavier path used for rare input or error-heavy situations

The important point is not only _what_ can be parsed, but _in what order_ and
at _what cost_ cases are parsed.

Two parsers may accept the same final input set:

- one checks all special cases eagerly
- the other handles common cases cheaply and only enters specialized logic when needed

The second one keeps the hot path cheaper. That is the direction `ox-jsdoc` should take.

#### A fast-path example in `ox-jsdoc`

For a normal comment such as:

```js
/**
 * Find a user.
 * @param {string} userId - The user ID
 * @returns {User}
 */
```

the fast path should roughly be:

1. read plain text description
2. detect `@param`
3. capture `{string}` as type
4. capture `userId` as value
5. capture `- The user ID` as description
6. process `@returns` similarly

The key is that the parser should not try to decide everything at this stage.

What it needs at this point:

- where the tags are
- where the type fragment begins and ends
- how the value / description split was made
- spans for each part

What it does not need yet:

- whether `userId` is semantically valid as a parameter path
- whether `User` should be fully normalized as a type-oriented name
- whether every mode-specific strict rule passes

Things to avoid on this path:

- deep custom tag dictionary lookup
- final strict mode resolution
- full namepath normalization
- heavy rollback logic
- trying every special tag parser for every tag

For example, when reading a normal `@param`, the parser should not also try:

- the `@borrows` parser
- the `@variation` parser
- the `@memberof!` parser
- custom-tag-specific rule logic

The fast path should follow a simple rule:
extract a generic shape first, look for special interpretation only when necessary.

#### Cases that should go to the cold path

Examples of less common inputs:

```js
/**
 * @variation 3
 * @memberof! Foo.
 * @borrows source as target
 * @customTag [foo=bar] baz qux
 */
```

Or malformed input:

```js
/**
 * @param {string foo
 */
```

In such cases, it is acceptable to:

- keep only the generic extraction and preserve `raw_body`
- decide special interpretation later
- run recovery and detailed diagnostics only on failure

More concretely, cold path entry conditions include:

- encountering an infrequent built-in tag
- seeing special markers such as `!`, `as`, or trailing connectors
- parse failure such as unclosed type braces or unstable name splitting
- needing custom tag dictionary lookup
- needing validator-driven deeper semantic interpretation

So the cold path is not only “the failure path”.
It is also the path taken when input is clearly outside the common shape.

#### Why this separation matters

Most real JSDoc comments stay within a small subset:

- plain text description
- `@param`
- `@returns`
- `@throws`
- `@deprecated`
- `{@link ...}`

If every one of these had to pay for:

- custom tag rules
- mode-specific branching
- strict validation
- fallback parsers
- expensive rollback

then common comments would become unnecessarily expensive.

Fast-path / cold-path separation exists to prevent rare cases from slowing down normal ones.

Another way to say this:

1. run a cheap generic parse first
2. move only the necessary cases into deeper interpretation

This does not lower correctness.
It protects throughput for the common case.

#### Implementation guidance for this separation

##### 1. Keep plain text on the shortest possible path

While reading normal description text, inline-tag and block-tag checks should stay minimal.

In practice, that means paying special attention only at boundaries where meaning may change,
such as `@`, `{@`, or line boundaries, instead of increasing branching on every character.

##### 2. Extract common built-in tags into a generic shape quickly

Frequent tags such as `@param`, `@returns`, and `@throws` should first be parsed
into something like `type? + value? + description?`.

At this stage, `JsdocTagBody::Generic` plus `raw_body` is often enough.
Specialized AST variants and strict normalization do not need to happen immediately.

##### 3. Push specialized interpretation backward

Strict handling of `@variation`, `@memberof!`, `@borrows`, and custom tags
should mostly live in validator logic.

Instead of attaching many special parsers directly to the parse hot path,
the parser should preserve the fact that “this tag may need deeper interpretation later”.

##### 4. Use rollback only where it is truly needed

Checkpoint / rewind behavior should be limited to genuinely ambiguous places.

Common flows such as:

- reading plain text description
- reading a closed `{...}` type
- reading `- description`

should avoid rollback-oriented control flow if possible.

##### 5. Build detailed diagnostics only on failure

Success paths should avoid heavy message construction and expensive supporting data collection.

Often it is enough on success to keep:

- an error code placeholder
- a span
- no fully constructed explanation string

Detailed messages, suggestions, and extra spans should be created only when failure actually occurs,
or in validator / analyzer phases.

##### 6. Make cold-path boundaries visible in the code

Fast-path / cold-path separation tends to erode during implementation.
It helps if the boundaries are visible through helper functions or explicit phase transitions.

Examples:

- `parse_generic_block_tag_body`
- `parse_borrows_body_if_applicable`
- `validate_special_tag_semantics`

This keeps it clear how far a normal comment travels through cheap logic before entering slower logic.

This does **not** mean recreating every byte-level lexer trick from `oxc`.
For `ox-jsdoc`, it means keeping the normal JSDoc path short and pushing exceptional handling later.
