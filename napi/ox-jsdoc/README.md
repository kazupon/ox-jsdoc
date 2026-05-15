# ox-jsdoc

High-performance JSDoc parser that returns a lazy Binary AST decoder.

`ox-jsdoc` parses `/** ... */` comment blocks on the Rust side and returns a single binary buffer. Node access on the JavaScript side is lazy: the [`@ox-jsdoc/decoder`](../../packages/decoder/) library walks the buffer on demand and only allocates JS objects for nodes the caller actually inspects. This minimizes allocation churn for large lint passes and unlocks **batch parsing** so common strings (`*`, `*/`, tag names) are interned once across many comments.

This is the canonical Binary AST NAPI binding (post-cutover, see [`design/010-main-stream-binary/README.md`](../../design/010-main-stream-binary/README.md)). The original typed AST + JSON implementation is preserved as the private reference package [`ox-jsdoc-origin`](../ox-jsdoc-origin/).

## Install

```sh
npm install ox-jsdoc
# for pnpm
pnpm add ox-jsdoc
# for yarn
yarn add ox-jsdoc
```

Pre-built binaries are published for:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

<!-- prettier-ignore -->
> [!NOTE]
> Node.js `^20.19.0 || >=22.12.0` is required.

<!-- prettier-ignore -->
> [!NOTE]
> The thin alias package [`ox-jsdoc-binary`](../ox-jsdoc-binary/) re-exports
> this package for one deprecation cycle to ease migration from the
> previously-published `ox-jsdoc-binary` `0.0.12`. New code should depend on
> `ox-jsdoc` directly.

## Usage

### `parse(sourceText, options?)`

Parse a single `/** ... */` JSDoc block comment.

```js
import { parse } from 'ox-jsdoc'

const { ast, diagnostics, sourceFile } = parse('/** @param {string} id - The user ID */')

console.log(ast.type) // 'JsdocBlock'
console.log(ast.tags[0].tag.value) // 'param'
console.log(ast.tags[0].rawType.raw) // 'string'
console.log(ast.tags[0].name.raw) // 'id'
console.log(ast.descriptionText()) // ''
```

Returns:

```ts
interface ParseResult {
  /** Lazy root `RemoteJsdocBlock`, or `null` on parse failure. */
  ast: RemoteJsdocBlock | null
  /** Parser diagnostics. */
  diagnostics: Array<{ message: string }>
  /** Underlying buffer wrapper — keep alive while reading from `ast`. */
  sourceFile: RemoteSourceFile
}
```

<!-- prettier-ignore -->
> [!IMPORTANT]
>  Hold onto `sourceFile` for as long as you read from `ast`. The `ast` getters lazily read from `sourceFile.view`, so once `sourceFile` is garbage collected the underlying buffer goes too.

### `parseBatch(items, options?)`

Parse N JSDoc block comments at once into a single shared Binary AST buffer. This is where this package earns its keep: a single NAPI boundary crossing, shared string table, shared allocation arena.

```js
import { parseBatch } from 'ox-jsdoc'

const { asts, diagnostics, sourceFile } = parseBatch([
  { sourceText: '/** @param {string} a */', baseOffset: 0 },
  { sourceText: '/** @param {number} b */', baseOffset: 100 },
  { sourceText: '/** @returns {void} */', baseOffset: 200 }
])

for (const ast of asts) {
  if (ast === null) continue // parse failure for this item
  console.log(ast.tags[0].tag.value)
}
```

`baseOffset` is the absolute byte offset of `sourceText` in the original source file — used so that `ast.range` etc. report positions relative to the host file rather than the slice. Defaults to `0`.

Each entry in `diagnostics` carries a `rootIndex` field pointing back to the input `items` index that produced it, so a single shared `diagnostics[]` can be attributed per-comment.

For consumers that want the [`@ox-jsdoc/jsdoccomment`](../../packages/jsdoccomment/) normalizer (`@es-joy/jsdoccomment` compatible shape), pass `output: 'jsdoccomment-input'`:

```js
const { blocks, diagnostics } = parseBatch(items, {
  output: 'jsdoccomment-input',
  compatMode: true,
  emptyStringForNull: true,
  preserveWhitespace: true
})
// `blocks[i]` is now an intermediate object that
// `@ox-jsdoc/jsdoccomment` can wrap into the compat-mode AST.
```

### `parseType(typeText, mode?)`

Parse a standalone JSDoc type expression and return its stringified form, or `null` if parsing fails.

```js
import { parseType } from 'ox-jsdoc'

parseType('string | number') // 'string | number'
parseType('Array<{ id: string }>') // 'Array<{ id: string }>'
parseType('not a type {{') // null
```

### `parseTypeCheck(typeText, mode?)`

Parse a JSDoc type expression and return `true`/`false` for success without the stringification overhead.

```js
import { parseTypeCheck } from 'ox-jsdoc'

parseTypeCheck('string | number') // true
parseTypeCheck('not a type {{') // false
```

`mode` is `'jsdoc' | 'closure' | 'typescript'` (default: `'jsdoc'`).

### `jsdocVisitorKeys`

Re-exported from `@ox-jsdoc/decoder` for ergonomics — drop into `eslint-visitor-keys` / `estraverse` style traversal.

```js
import { jsdocVisitorKeys } from 'ox-jsdoc'
```

## Options

`parse(sourceText, options)` and `parseBatch(items, options)` accept:

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `fenceAware` | `boolean` | `true` | Suppress tag recognition inside fenced code blocks (` ``` `). |
| `parseTypes` | `boolean` | `false` | Parse `{...}` type expressions in tags into a structured `parsedType` AST. |
| `typeParseMode` | `'jsdoc' \| 'closure' \| 'typescript'` | `'jsdoc'` | Syntax flavor for type expressions when `parseTypes` is on. |
| `compatMode` | `boolean` | `false` | Enable `@es-joy/jsdoccomment` compat extension fields (`delimiter`, `postDelimiter`, line indices, …). |
| `preserveWhitespace` | `boolean` | `false` | Emit per-node `description_raw_span` so `descriptionRaw` getter and `descriptionText(true)` work. Adds 8 bytes per `JsdocBlock`/`JsdocTag` with description. |
| `emptyStringForNull` | `boolean` | `false` | In `toJSON()` output, convert absent optional strings to `""` instead of `null`. `compatMode` only. |
| `baseOffset` | `number` | `0` | Absolute byte offset of `sourceText` in the original file (per-item for `parseBatch`). |
| `output` | `'ast' \| 'jsdoccomment-input'` | `'ast'` | `parseBatch` only. Selects the materialized output. `'jsdoccomment-input'` returns intermediate `blocks[]` for the `@ox-jsdoc/jsdoccomment` normalizer. |

## Why a Binary AST?

For a single comment, the cost of building a JSON AST in JS is small. For **lint passes** that touch thousands of comments per project, three things add up:

1. **NAPI boundary crossings** — one per `parse` call.
2. **Per-AST allocations** — each comment becomes ~10-50 small JS objects.
3. **String duplication** — every `"param"`, `"returns"`, `"*"` is its own String allocation.

`parseBatch` collapses all three:

- **One** NAPI call for N comments
- **One** shared `ArrayBuffer` for all parsed nodes (no per-AST JS allocs until you traverse)
- **One** intern table for repeated strings inside the buffer

The lazy decoder ([`@ox-jsdoc/decoder`](../../packages/decoder/)) walks the buffer only when the consumer reads a field. Sparse access (e.g. only checking `descriptionText` on a few blocks) is essentially free.

See `design/008-oxlint-oxfmt-support/README.md` and the design notes under `design/007-binary-ast/` for the wire format and decoder design.

## Related packages

| Need | Use |
| --- | --- |
| Lazy Binary AST + batch parsing on Node.js | **`ox-jsdoc`** (this package, canonical) |
| Same Binary AST API in the browser via WebAssembly | [`@ox-jsdoc/wasm`](../../wasm/ox-jsdoc/) |
| `@es-joy/jsdoccomment` compatible AST shape | `@ox-jsdoc/jsdoccomment` (workspace package, builds on this binding) |
| Original typed AST + JSON serialization (reference / benchmark only) | [`ox-jsdoc-origin`](../ox-jsdoc-origin/) (private) |
| Migration alias from `ox-jsdoc-binary` `0.0.12` (one deprecation cycle) | [`ox-jsdoc-binary`](../ox-jsdoc-binary/) (re-exports this package) |

## Build from source

This package is built with [`@napi-rs/cli`](https://napi.rs/):

```sh
pnpm build       # release build
pnpm build-dev   # debug build (faster compile, slower runtime)
pnpm test        # run vitest tests
```

> [!NOTE] The compiled native module is dropped into `src-js/<binary>.node` and is not checked in.

## License

[MIT](https://opensource.org/licenses/MIT)
