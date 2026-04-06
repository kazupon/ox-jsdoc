# What Not to Bring In Yet

## 1. A Core Design Centered on Raw Transfer

This should not be a first-class concern for now.

Reasons:

- Raw transfer is transport-specific.
- It comes with strong runtime and platform constraints.
- It increases generator and ABI maintenance cost.
- JSDoc ASTs are usually much smaller than JS/TS ASTs.

Therefore:

- The AST should stay **raw-transfer friendly**.
- But raw transfer should not distort the core AST design.

In other words, a stable-ish layout is desirable, but transport-only fields
should not be inserted into the core AST.

## 2. Ultra-Low-Level Lexer Micro-Optimizations

In `oxc`, the following are well justified for a JS/TS parser:

- packed `Token`
- byte-handler jump tables
- UTF-8-safe search tables
- aggressive pointer walking

For `ox-jsdoc` v1, the cost-benefit tradeoff is likely different.
JSDoc parsing has a different input size and a different grammar profile.

What should be borrowed here is the optimization mindset, not the exact implementation.

A reasonable starting point is:

- a straightforward scanner
- control flow that keeps branches under control
- arena-backed output

Low-level micro-optimizations should be considered only after profiling shows
that they are worth the complexity.

## 3. Strong Semantic Graph Identity Inside the Core AST

`oxc` has a rich semantic graph with concepts such as `NodeId`, `ScopeId`,
`SymbolId`, and `ReferenceId`.

> [!NOTE]
> This does not refer only to lint-specific data.
> It includes analysis-oriented IDs and graph information such as semantic IDs,
> def-use relationships, cross-reference tracking, scope / symbol resolution
> results, and other higher-level analysis data used by validators, analyzers,
> linters, or formatters.
> The design direction here is to keep such data outside the core AST itself.

At the current stage, `ox-jsdoc` does not need to ship that as part of the
core AST by default.
If later we need JSDoc cross-reference resolution, integration with the code AST,
analysis caches for linting or formatting, or IDE-style reference features,
those can be added as outer-layer data structures.

For now:

- keep the AST self-contained and parse-oriented
- keep cross-node ownership and analysis identity in outer layers

This keeps the core AST lean.

## 4. Transport Complexity Inside Parser Core

`oxc`'s NAPI parser supports sync / async and eager / lazy transfer modes.
That is an integration-layer concern, not a parser-core concern.

`ox-jsdoc` should keep these layers separate:

- parser core
- validator / analyzer
- serializer / JS bridge
