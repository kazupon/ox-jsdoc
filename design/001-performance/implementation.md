# Implementation Guidance

## Concrete Directions for `ox-jsdoc`

### Direction 1. The parser builds ASTs, but does not decide final meaning

The parser should stop after producing a syntax-informed tree.

Examples:

- `@variation 3` can remain generic at parse time
- `@memberof! Foo.` can remain generic with preserved raw structure
- custom tags should be parsed as far as possible instead of being rejected early

### Direction 2. Keep expensive normalization for later phases

Tag-specific, mode-specific, and context-sensitive rules are often better handled
after parsing. The parser should preserve enough information for later analysis
instead of eagerly normalizing everything.

Examples:

- `raw_body`
- `TagValueToken`
- broad `NamePathLike`
- broad `ParameterPath`

### Direction 3. Split nodes when their semantic roles differ

If two syntactic forms play different semantic roles, they should not be merged
just because their surface syntax looks similar.

Examples:

- `NamePathLike` vs `ParameterPath`
- `TypeName` as a wrapper around `NamePathLike`
- a dedicated `BorrowsTagBody`

This is close in spirit to how `oxc` distinguishes identifier roles.

### Direction 4. Mechanically verify layout-sensitive parts

Once the AST stabilizes, at least representative nodes should be checked for:

- struct size
- enum size
- performance-sensitive field ordering

The goal is not to freeze an ABI too early.
The goal is to detect accidental regressions.

### Direction 5. Treat serializer shape as part of the design, even with JSON transfer

Even if JS transfer starts with JSON, serialization should not be an afterthought.

Desired properties:

- a serializer shape that is easy to reason about
  - it should be easy to see which node exposes which fields
- predictable field ordering
- limited temporary allocation

Even before raw transfer exists, the serializer is part of the performance story.

## Near-Term Recommended Architecture

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

## Performance Measurement Strategy

Performance work should be measurement-driven.
`ox-jsdoc` should avoid speculative optimization and instead decide based on
repeatable measurements against realistic inputs.

### What should be measured

At minimum, the project should measure:

1. **Parse throughput**
   - time to parse comments into `JSDocComment<'a>`
2. **Validation throughput**
   - time to apply tag-specific and mode-specific rules on top of parsed AST
3. **Serialization throughput**
   - time to convert AST into the JSON shape exposed to JS
4. **Allocation behavior**
   - total allocation count or allocated bytes where practical
5. **Representative node layout**
   - size-sensitive regressions in core AST nodes

This keeps the project honest about where time is actually spent.

### What should not be measured only in aggregate

A single end-to-end benchmark is not enough.
At least three levels should exist:

1. **Micro benchmarks**
   - scanner-only or parser-only hot paths
   - common tag shapes such as `@param`, `@returns`, `@throws`
   - common description parsing with and without inline tags

2. **Component benchmarks**
   - parser only
   - parser + validator
   - parser + validator + serializer

3. **Corpus benchmarks**
   - realistic multi-comment inputs collected from real-world JSDoc usage

If everything is measured only end-to-end, it becomes difficult to tell whether
a regression comes from scanning, AST construction, validation, or serialization.

### Recommended benchmark corpus

The benchmark corpus should include at least these buckets:

- **Common comments**
  - short descriptions
  - `@param`
  - `@returns`
  - `@throws`
  - `@deprecated`
- **Description-heavy comments**
  - multiple inline tags
  - long text segments
  - fenced code blocks
- **Type-heavy comments**
  - nested generics
  - indexed access
  - record-like structures
  - mode-sensitive type syntax
- **Special-tag comments**
  - `@variation`
  - `@memberof!`
  - `@borrows`
  - custom tags
- **Malformed comments**
  - unclosed type braces
  - incomplete inline tags
  - ambiguous or partially broken name/value splits

This matters because an optimization that helps `@param` may hurt long descriptions,
and an optimization that helps simple types may hurt malformed recovery.

### Comparison baselines

`ox-jsdoc` should not rely on only one comparison target.
Its benchmark strategy should distinguish between external baselines and internal baselines.

#### External baselines

1. **`comment-parser`**
   - primary parser-level baseline
   - useful for comparing raw comment parsing cost against a widely used JavaScript implementation

2. **`jsdoc`**
   - ecosystem-level reference point
   - useful for understanding how `ox-jsdoc` compares to a more traditional JSDoc processing stack

3. **toolchain-oriented baselines**
   - useful for understanding the value of `ox-jsdoc` in realistic linting workflows
   - likely future candidates include:
     - `eslint-plugin-jsdoc` in an ESLint-based pipeline
     - an `oxlint`-oriented dedicated plugin or JSDoc analysis integration built on top of `ox-jsdoc`

These two baselines are useful for different reasons:

- `comment-parser` is closer to a parsing-focused comparison
- `jsdoc` is closer to an ecosystem / workflow comparison

The toolchain-oriented baselines answer a different question:

- how much benefit does `ox-jsdoc` provide once parsing is embedded into a real linting or analysis pipeline
- how much parser-level speed survives after rule execution, validation, and integration overhead are included

Neither is a perfect apples-to-apples match:

- `comment-parser` does not build the same kind of AST richness
- `jsdoc` is not just a parser and includes broader processing concerns
- toolchain comparisons include rule-engine and integration costs that are not parser-only costs

So the comparison should not be framed as a simplistic “faster or slower than X”.
It should answer more useful questions such as:

- how expensive is `ox-jsdoc` on common comments
- how much overhead comes from richer AST structure
- how much malformed-input handling changes the cost
- how much end-to-end benefit remains when `ox-jsdoc` is used inside a practical toolchain

#### Internal baselines

The more important regression guard is `ox-jsdoc` itself, measured stage by stage:

1. scanner only
2. parser only
3. parser + validator
4. parser + validator + serializer

This is necessary because external comparisons alone do not show where regressions come from.
If performance drops, the project needs to know whether the regression came from:

- scanning
- AST construction
- validation
- serialization

The internal baseline should therefore be the primary tool for day-to-day optimization work,
while external baselines should serve as contextual reference points.

#### Recommended adoption order for external comparison

External comparisons do not all need to exist from day one.
A practical order is:

1. `comment-parser` as the first parser-level baseline
2. `jsdoc` as an ecosystem-level reference
3. toolchain-level comparisons once `ox-jsdoc` is integrated into lint-oriented workflows

This order keeps early measurement simple while leaving room to demonstrate
the larger practical advantage of `ox-jsdoc` later.

### Primary comparison rules

Performance changes should be evaluated with a few stable questions:

- Does this improve the common parser hot path?
- Does this regress malformed-input recovery too much?
- Does this increase node size on common AST paths?
- Does this move work from parser to validator, and is that shift acceptable?
- Does this improve throughput only by losing useful source fidelity?

This is important because not every speedup is a good trade.
For example, flattening description structure may speed up parsing a little while
hurting linting, formatting, and diagnostics.

### Initial tooling direction

The exact framework can evolve, but the first implementation should support:

- repeatable Rust benchmarks for parser / validator / serializer stages
- fixed sample inputs checked into the repository
- simple memory-oriented regression checks where feasible
- CI-visible regression tracking for representative cases

The main goal is not to build a large benchmarking system immediately.
The main goal is to make performance claims testable from the beginning.

### Recommended adoption order

1. Add parser-only micro benchmarks for common comments.
2. Add parser-only corpus benchmarks using real-world comment sets.
3. Add validator benchmarks once tag semantics become non-trivial.
4. Add serializer benchmarks once the JSON shape stabilizes.
5. Add lightweight regression checks in CI for representative cases.

This order follows the implementation order of the system itself.

### Practical rule for optimization work

No low-level optimization should be adopted just because it looks fast in theory.
Before adding complexity such as specialized scanners, extra caching, or more
aggressive AST packing, the project should be able to show:

- which benchmark regressed
- which phase regressed
- which representative inputs regressed
- what tradeoff is being accepted

That rule is the part of the `oxc` performance mindset most worth copying here.

## Adoption Summary

Adopt now:

- arena-backed AST
- span-rich nodes
- parser / semantic separation
- node design that respects compact layout
- preservation of enough raw syntax for later validation
- generated code or mechanical checks where invariants matter

Defer:

- raw transfer
- fixed transport ABI
- deep lexer micro-optimization
- heavy semantic graph IDs inside the core AST

Avoid:

- validating every tag rule inside the parser
- leaking transport-specific constraints into the core AST
- overfitting to `oxc` implementation details that are justified only at JS/TS parser scale

## Conclusion

`ox-jsdoc` should be **oxc-inspired**, not **oxc-cloned**.

What matters most is the performance philosophy:

- a lean hot path
- clear phase boundaries
- compact memory layout
- mechanical protection for structural invariants

The most specialized implementation techniques should be introduced only when
measurement shows that JSDoc parsing actually needs them.
