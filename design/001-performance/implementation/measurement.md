# Performance Measurement Strategy

Performance work should be measurement-driven.
`ox-jsdoc` should avoid speculative optimization and instead decide based on
repeatable measurements against realistic inputs.

## What should be measured

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

## What should not be measured only in aggregate

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

## Recommended benchmark corpus

The benchmark corpus should include at least these buckets:

- **Common comments**
  - short descriptions
  - `@param`
  - `@returns`
  - `@throws`
  - `@deprecated`
- **Description-heavy comments**
  - multiple inline tags
  - `{@linkcode Foo | label}`-style inline tags
  - long text segments
  - fenced code blocks
  - long `@remarks`
- **Type-heavy comments**
  - nested generics
  - indexed access
  - record-like structures
  - mode-sensitive type syntax
  - TypeScript-like `import(...) & { ... }` shapes
- **Special-tag comments**
  - `@variation`
  - `@memberof!`
  - `@borrows`
  - `@typeParam`
  - custom tags
- **Malformed comments**
  - unclosed type braces
  - incomplete inline tags
  - ambiguous or partially broken name/value splits

This matters because an optimization that helps `@param` may hurt long descriptions,
and an optimization that helps simple types may hurt malformed recovery.

## Comparison baselines

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

## Fixture strategy

Parser fixtures should not come from only one source.
`ox-jsdoc` needs fixtures that balance:

- compatibility with canonical JSDoc behavior
- robustness against raw parsing edge cases
- realism inside linting-oriented toolchains

#### Recommended source roles

1. **`refers/jsdoc`**
   - canonical compatibility source
   - use it for representative built-in tag behavior and JSDoc-oriented parsing cases
   - especially useful for tags such as `@param`, `@returns`, `@throws`, `@variation`, `@memberof!`, and `@borrows`

2. **`refers/comment-parser`**
   - raw parsing edge-case source
   - use it for multiline type parsing, optional/default names, malformed-but-recoverable input, and inline / fence boundary behavior

3. **`refers/eslint-plugin-jsdoc`**
   - real-world and toolchain-oriented source
   - use it for parameter-path shapes, mode-sensitive type syntax, escaping behavior, and practical lint-driven comment patterns

`refers/jsdoc` alone is important, but it is not sufficient.
The fixture set should reflect both parser correctness and ecosystem realism.

#### Recommended adoption order

1. Start with `refers/jsdoc` fixtures/specs as the primary parser-compatibility source.
2. Add `refers/comment-parser` cases to harden raw parsing behavior and recovery.
3. Add `refers/eslint-plugin-jsdoc` cases once parser and validator behavior are stable enough to test realistic toolchain inputs.

Fixture planning should mirror benchmark planning.
The suite should intentionally cover:

- common cases
- special-tag cases
- malformed cases
- toolchain-oriented real-world cases

#### Fixture file layout

Performance fixtures should use sidecar JSON metadata.
The `.jsdoc` file should contain only the exact bytes passed to the parser.
Metadata should live in a sibling `.json` file with the same basename.

Recommended layout:

```text
fixtures/
  perf/
    common/
      basic-param.jsdoc
      basic-param.json
    description-heavy/
      linkcode-description.jsdoc
      linkcode-description.json
    type-heavy/
      ts-import-record-type.jsdoc
      ts-import-record-type.json
    special-tag/
      type-param.jsdoc
      type-param.json
    malformed/
      unclosed-inline-tag.jsdoc
      unclosed-inline-tag.json
    toolchain/
      vue-i18n-custom-tags.jsdoc
      vue-i18n-custom-tags.json
```

Example `.jsdoc` fixture:

```jsdoc
/**
 * See {@link UserService for details.
 */
```

Example sidecar `.json` metadata:

```json
{
  "description": "Unclosed inline link should recover as text.",
  "source": "refers/comment-parser",
  "sourcePath": "refers/comment-parser/test/...",
  "category": "malformed",
  "features": ["inline-tag", "recovery"],
  "expect": {
    "parse": "recoverable",
    "diagnostics": ["unclosed_inline_tag"]
  }
}
```

Use sidecar JSON instead of YAML frontmatter because span and byte-offset tests
must parse the fixture input exactly as-is.
Frontmatter would shift offsets and force benchmark code to strip metadata
before parsing.

The metadata format should stay small at first:

- `description`
- `source`
- `sourcePath`
- `category`
- `features`
- `expect.parse`
- `expect.diagnostics`

#### Recommended adoption order for external comparison

External comparisons do not all need to exist from day one.
A practical order is:

1. `comment-parser` as the first parser-level baseline
2. `jsdoc` as an ecosystem-level reference
3. toolchain-level comparisons once `ox-jsdoc` is integrated into lint-oriented workflows

This order keeps early measurement simple while leaving room to demonstrate
the larger practical advantage of `ox-jsdoc` later.

## Primary comparison rules

Performance changes should be evaluated with a few stable questions:

- Does this improve the common parser hot path?
- Does this regress malformed-input recovery too much?
- Does this increase node size on common AST paths?
- Does this move work from parser to validator, and is that shift acceptable?
- Does this improve throughput only by losing useful source fidelity?

This is important because not every speedup is a good trade.
For example, flattening description structure may speed up parsing a little while
hurting linting, formatting, and diagnostics.

## Initial tooling direction

The initial Rust benchmark framework should be **`criterion2`**.

Reasons:

- it matches the direction used by `refers/oxc/tasks/benchmark`
- it supports normal local `cargo bench` workflows
- it can be connected to CodSpeed later through the `criterion2/codspeed` feature
- it is sufficient for parser / validator / serializer stage benchmarks
- it avoids selecting a less proven benchmark stack before the parser exists

`divan` should not be the first benchmark framework.
It can be revisited later if the project needs a simpler benchmark harness or
if measurement shows that `criterion2` overhead / ergonomics becomes a problem.

The first benchmark crate or module should support:

- repeatable Rust benchmarks for parser / validator / serializer stages
- fixed sample inputs checked into the repository
- simple memory-oriented regression checks where feasible
- CI-visible regression tracking for representative cases

The main goal is not to build a large benchmarking system immediately.
The main goal is to make performance claims testable from the beginning.

Recommended initial layout once the Rust workspace exists:

```text
benches/
  parser.rs
  validator.rs
  serializer.rs
```

The first parser benchmark should measure:

- common short comments
- description-heavy comments
- type-heavy comments
- malformed recovery comments
- fixture corpus parsing

CodSpeed integration should be deferred until benchmarks are stable enough to be
useful in CI. Local benchmark repeatability should come first.

## Post-measurement performance design items

The following items should remain explicit deferred design topics.
They should not be adopted before parser benchmarks and representative fixtures
exist.

1. **`Ident`-like pre-hashed tag names**
   - revisit if tag lookup or tag-name comparison appears in hot profiles
   - default v1 representation remains borrowed `&'a str`

2. **byte-search tables / SIMD**
   - revisit if scanner marker search dominates parser time
   - default v1 scanner remains byte-oriented but straightforward

3. **public token / event stream**
   - revisit if external consumers need lower-level data or if recovery becomes simpler with a stable event layer
   - default v1 API remains `parse_comment(...) -> ParseOutput<'a>`

4. **raw transfer layout**
   - revisit after the JSON serializer shape is stable and JS transfer cost is measured
   - default v1 transfer remains JSON-first

5. **AST layout thresholds**
   - revisit after real Rust node sizes exist
   - default v1 only tracks representative node layout qualitatively

6. **switching from `criterion2` to `divan`**
   - revisit if `criterion2` overhead or ergonomics becomes a practical problem
   - default v1 benchmark framework remains `criterion2`

## Recommended adoption order

1. Add parser-only micro benchmarks for common comments.
2. Add parser-only corpus benchmarks using real-world comment sets.
3. Add validator benchmarks once tag semantics become non-trivial.
4. Add serializer benchmarks once the JSON shape stabilizes.
5. Add lightweight regression checks in CI for representative cases.
6. Add CodSpeed integration after benchmark names and fixture buckets are stable.

This order follows the implementation order of the system itself.

## Practical rule for optimization work

No low-level optimization should be adopted just because it looks fast in theory.
Before adding complexity such as specialized scanners, extra caching, or more
aggressive AST packing, the project should be able to show:

- which benchmark regressed
- which phase regressed
- which representative inputs regressed
- what tradeoff is being accepted

That rule is the part of the `oxc` performance mindset most worth copying here.
