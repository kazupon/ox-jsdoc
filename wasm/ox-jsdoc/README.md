# @ox-jsdoc/wasm

High-performance JSDoc parser for the browser, powered by WebAssembly, that returns a lazy Binary AST decoder.

`@ox-jsdoc/wasm` parses `/** ... */` comment blocks on the Rust (WASM) side and returns a single binary buffer. Node access on the JavaScript side is lazy: the [`@ox-jsdoc/decoder`](../../packages/decoder/) library walks the buffer on demand and only allocates JS objects for nodes the caller actually inspects. This minimizes allocation churn for large batch passes and unlocks **batch parsing** so common strings (`*`, `*/`, tag names) are interned once across many comments.

It exposes the same API as the canonical native [`ox-jsdoc`](../../napi/ox-jsdoc/) NAPI binding, but runs in any browser environment via a WebAssembly binary built from the same Rust core (`crates/ox_jsdoc`).

This is the canonical Binary AST WASM binding (post-cutover, see [`design/010-main-stream-binary/README.md`](../../design/010-main-stream-binary/README.md)). The original typed AST + JSON implementation is preserved as the private reference package [`@ox-jsdoc/wasm-origin`](../ox-jsdoc-origin/).

## Install

```sh
npm install @ox-jsdoc/wasm
# or
pnpm add @ox-jsdoc/wasm
# or
yarn add @ox-jsdoc/wasm
```

<!-- prettier-ignore -->
> [!NOTE]
> The thin alias package [`@ox-jsdoc/wasm-binary`](../ox-jsdoc-binary/)
> re-exports this package for one deprecation cycle to ease migration from
> the previously-published `@ox-jsdoc/wasm-binary` `0.0.12`. New code should
> depend on `@ox-jsdoc/wasm` directly.

## Usage

The WASM module must be initialized once before calling `parse()` / `parseBatch()`.

### `parse(sourceText, options?)`

Parse a single `/** ... */` JSDoc block comment.

```js
import { initWasm, parse } from '@ox-jsdoc/wasm'

await initWasm()

const result = parse(`/**
 * Look up a user by ID.
 * @param {string} id - The user ID
 */`)

console.log(result.ast.type) // 'JsdocBlock'
console.log(result.ast.descriptionText()) // 'Look up a user by ID.'
console.log(result.ast.tags[0].tag.value) // 'param'  (RemoteJsdocTagName.value)
console.log(result.ast.tags[0].rawType.raw) // 'string' (RemoteJsdocTypeSource.raw)
console.log(result.ast.tags[0].name.raw) // 'id'    (RemoteJsdocTagNameValue.raw)

// Release the WASM-side bytes when you're done reading from `ast`.
result.free()
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
  /** Release the WASM-side bytes. After calling, `ast` / `sourceFile` are unsafe to read. */
  free(): void
}
```

<!-- prettier-ignore -->
> [!Important]
> 1. Hold onto `sourceFile` for as long as you read from `ast`. The `ast` getters lazily read from `sourceFile.view`, so once `sourceFile` is garbage collected the underlying buffer goes too.
> 2. Call `free()` once you no longer need the AST. WASM linear memory is not garbage collected on the JS side, so leaving buffers around will pin memory that can only be reclaimed when the whole WASM module is discarded.

### `parseBatch(items, options?)`

Parse N JSDoc block comments at once into a single shared Binary AST buffer. This is where this package earns its keep: a single WASM boundary crossing, shared string table, shared allocation arena.

```js
import { initWasm, parseBatch } from '@ox-jsdoc/wasm'

await initWasm()

const result = parseBatch([
  { sourceText: '/** @param {string} a */', baseOffset: 0 },
  { sourceText: '/** @param {number} b */', baseOffset: 100 },
  { sourceText: '/** @returns {void} */', baseOffset: 200 }
])

for (const ast of result.asts) {
  if (ast === null) continue // parse failure for this item
  console.log(ast.tags[0].tag.value)
}

result.free() // release WASM-side bytes
```

`baseOffset` is the absolute byte offset of `sourceText` in the original source file — used so that `ast.range` etc. report positions relative to the host file rather than the slice. Defaults to `0`.

For consumers that want the [`@ox-jsdoc/jsdoccomment`](../../packages/jsdoccomment/) normalizer (`@es-joy/jsdoccomment` compatible shape), pass `output: 'jsdoccomment-input'`:

```js
const { blocks, diagnostics } = parseBatch(items, {
  output: 'jsdoccomment-input',
  compatMode: true,
  preserveWhitespace: true
})
// `blocks[i]` is now an intermediate object that
// `@ox-jsdoc/jsdoccomment` can wrap into the compat-mode AST.
```

### `parseType(typeText, mode?)`

Parse a standalone JSDoc type expression and return its stringified form, or `null` if parsing fails.

```js
import { initWasm, parseType } from '@ox-jsdoc/wasm'

await initWasm()

parseType('string | number') // 'string | number'
parseType('Array<{ id: string }>') // 'Array<{ id: string }>'
parseType('not a type {{') // null
```

### `parseTypeCheck(typeText, mode?)`

Parse a JSDoc type expression and return `true`/`false` for success without the stringification overhead.

```js
import { initWasm, parseTypeCheck } from '@ox-jsdoc/wasm'

await initWasm()

parseTypeCheck('string | number') // true
parseTypeCheck('not a type {{') // false
```

`mode` is `'jsdoc' | 'closure' | 'typescript'` (default: `'jsdoc'`).

### `jsdocVisitorKeys`

Re-exported from `@ox-jsdoc/decoder` for ergonomics — drop into `eslint-visitor-keys` / `estraverse` style traversal.

```js
import { jsdocVisitorKeys } from '@ox-jsdoc/wasm'
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
| `baseOffset` | `number` | `0` | Absolute byte offset of `sourceText` in the original file (per-item for `parseBatch`). |

## Why a Binary AST?

For a single comment, the cost of building a JSON AST in JS is small. For **lint passes** that touch thousands of comments per project, three things add up:

1. **WASM boundary crossings** — one per `parse` call.
2. **Per-AST allocations** — each comment becomes ~10-50 small JS objects.
3. **String duplication** — every `"param"`, `"returns"`, `"*"` is its own String allocation.

`parseBatch` collapses all three:

- **One** WASM call for N comments
- **One** shared `ArrayBuffer` for all parsed nodes (no per-AST JS allocs until you traverse)
- **One** intern table for repeated strings inside the buffer

The lazy decoder ([`@ox-jsdoc/decoder`](../../packages/decoder/)) walks the buffer only when the consumer reads a field. Sparse access (e.g. only checking `descriptionText` on a few blocks) is essentially free.

See `design/008-oxlint-oxfmt-support/README.md` and the design notes under `design/007-binary-ast/` for the wire format and decoder design.

## Related packages

| Need | Use |
| --- | --- |
| Lazy Binary AST + batch parsing in the browser | **`@ox-jsdoc/wasm`** (this package, canonical) |
| Same Binary AST API on Node.js (no `initWasm` needed) | [`ox-jsdoc`](../../napi/ox-jsdoc/) |
| `@es-joy/jsdoccomment` compatible AST shape | `@ox-jsdoc/jsdoccomment` (workspace package) |
| Original typed AST + JSON serialization (reference / benchmark only) | [`@ox-jsdoc/wasm-origin`](../ox-jsdoc-origin/) (private) |
| Migration alias from `@ox-jsdoc/wasm-binary` `0.0.12` (one deprecation cycle) | [`@ox-jsdoc/wasm-binary`](../ox-jsdoc-binary/) (re-exports this package) |

## Build from source

This package is built with [`wasm-pack`](https://rustwasm.github.io/wasm-pack/):

```sh
pnpm build         # debug build (faster compile, slower runtime)
pnpm build:release # release build (slower compile, optimized runtime)
pnpm test          # run vitest tests
```

The compiled WASM module is dropped into `pkg/` and is not checked in.

## License

[MIT](https://opensource.org/licenses/MIT)
