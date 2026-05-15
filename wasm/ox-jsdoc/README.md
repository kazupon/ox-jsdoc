# @ox-jsdoc/wasm-binary

High-performance JSDoc parser for the browser, powered by WebAssembly, that returns a lazy Binary AST decoder.

`@ox-jsdoc/wasm-binary` parses `/** ... */` comment blocks on the Rust (WASM) side and returns a single binary buffer. Node access on the JavaScript side is lazy: the [`@ox-jsdoc/decoder`](../../packages/decoder/) library walks the buffer on demand and only allocates JS objects for nodes the caller actually inspects. This minimizes allocation churn for large batch passes and unlocks **batch parsing** so common strings (`*`, `*/`, tag names) are interned once across many comments.

It exposes the same API as the native [`ox-jsdoc-binary`](../../napi/ox-jsdoc-binary/) NAPI binding, but runs in any browser environment via a WebAssembly binary built from the same Rust core (`crates/ox_jsdoc_binary`).

For a plain JSON AST that's eagerly materialized in JS, use the sibling package [`@ox-jsdoc/wasm`](../ox-jsdoc/) instead.

<!-- prettier-ignore -->
> [!WARNING]
> This package is planned to be **discontinued** in the near future. The Binary AST decoder implementation that lives here will be merged into [`ox-jsdoc`](../ox-jsdoc/), which will be rebuilt on top of it. Once that migration lands, `ox-jsdoc-binary` will stop receiving updates and is expected to be deprecated. New code should plan to depend on `ox-jsdoc` directly; existing users should pin a version and watch the release notes for the migration path.
>
> Until then, `ox-jsdoc-binary` is kept published primarily as a **performance reference** — a way to benchmark the Binary AST / lazy decoder path side-by-side against the plain JSON AST path exposed by `ox-jsdoc`, and to validate the perf gains before they are absorbed into `ox-jsdoc` itself.

## Install

```sh
npm install @ox-jsdoc/wasm-binary
# or
pnpm add @ox-jsdoc/wasm-binary
# or
yarn add @ox-jsdoc/wasm-binary
```

## Usage

The WASM module must be initialized once before calling `parse()` / `parseBatch()`.

### `parse(sourceText, options?)`

Parse a single `/** ... */` JSDoc block comment.

```js
import { initWasm, parse } from '@ox-jsdoc/wasm-binary'

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
import { initWasm, parseBatch } from '@ox-jsdoc/wasm-binary'

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

### `jsdocVisitorKeys`

Re-exported from `@ox-jsdoc/decoder` for ergonomics — drop into `eslint-visitor-keys` / `estraverse` style traversal.

```js
import { jsdocVisitorKeys } from '@ox-jsdoc/wasm-binary'
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

## When to use which package

| Need | Use |
| --- | --- |
| Plain JSON AST, immediate access to all fields | [`@ox-jsdoc/wasm`](../ox-jsdoc/) (typed AST WASM binding) |
| Lazy access, lowest allocation, batch parsing | **`@ox-jsdoc/wasm-binary`** (this package) |
| Same API but in Node.js (no `initWasm` needed) | [`ox-jsdoc-binary`](../../napi/ox-jsdoc-binary/) (NAPI binding) |
| `@es-joy/jsdoccomment` compatible AST shape | `@ox-jsdoc/jsdoccomment` (workspace package, builds on these bindings) |

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
