# ox-jsdoc

High-performance JSDoc parser powered by Rust and NAPI.

`ox-jsdoc` parses `/** ... */` comment blocks into a plain JSON AST that is ergonomic to consume from JavaScript or TypeScript. The parser core is implemented in Rust (`crates/ox_jsdoc`) and exposed through a NAPI binding, so you get native parsing speed with no JavaScript-side parser code path.

For lazy / zero-copy access (lower allocation, batch parsing, multi-comment amortization), see the sibling package [`ox-jsdoc-binary`](../ox-jsdoc-binary/) which exposes the same Rust core through a Binary AST decoder.

<!-- prettier-ignore -->
> [!WARNING]
> In the near future, this package will be rebuilt on top of [`ox-jsdoc-binary`](../ox-jsdoc-binary/), which is a **breaking change**. Specifically:
>
> - `parse` will return a `sourceFile` handle in addition to `ast` / `diagnostics`, and the `ast` will become a lazy decoder node (`RemoteJsdocBlock`) rather than a plain JSON object — field access like `ast.tags[0].tag` will become `ast.tags[0].tag.value`.
> - Plain-JSON-only options such as `includePositions` and `spacing` may be removed or replaced.
> - `parseType` / `parseTypeCheck` are not currently exposed by `ox-jsdoc-binary` and may be removed, renamed, or relocated.
>
> If you depend on the current shape, pin a version and watch the release notes before upgrading.

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

## Usage

### `parse(sourceText, options?)`

Parse a complete `/** ... */` JSDoc block comment.

```js
import { parse } from 'ox-jsdoc'

const { ast, diagnostics } = parse('/** @param {string} id - The user ID */')

console.log(ast.tags[0].tag) // 'param'
console.log(ast.tags[0].rawType) // 'string'
console.log(ast.tags[0].name) // 'id'
console.log(ast.tags[0].description) // 'The user ID'
console.log(diagnostics) // []
```

`parse` returns:

```ts
interface ParseResult {
  /** Parsed AST as a JSON object (ESTree-like shape), or null on fatal error. */
  ast: JsdocBlock | null
  /** Parser diagnostics. Empty on successful parse. */
  diagnostics: Array<{ message: string }>
}
```

The full AST shape (`JsdocBlock`, `JsdocTag`, `JsdocDescriptionLine`, `JsdocInlineTag`, etc.) is documented in [`src-js/index.d.ts`](./src-js/index.d.ts).

### `parseType(typeText, mode?)`

Parse a standalone JSDoc type expression and return its stringified form, or `null` if parsing fails.

```js
import { parseType } from 'ox-jsdoc'

parseType('string | number') // 'string | number'
parseType('Array<{ id: string }>') // 'Array<{ id: string }>'
parseType('not a type {{') // null
```

### `parseTypeCheck(typeText, mode?)`

Parse a JSDoc type expression and return `true`/`false` for success without the stringification overhead. Useful when you only need to validate that a type expression is well-formed.

```js
import { parseTypeCheck } from 'ox-jsdoc'

parseTypeCheck('string | number') // true
parseTypeCheck('not a type {{') // false
```

`mode` is `'jsdoc' | 'closure' | 'typescript'` (default: `'jsdoc'`). Selects the syntax flavor for type expressions.

## Options

`parse(sourceText, options)` accepts:

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `fenceAware` | `boolean` | `true` | Suppress tag recognition inside fenced code blocks (` ``` `). |
| `parseTypes` | `boolean` | `false` | Parse `{...}` type expressions in tags into a structured `parsedType` AST (`jsdoc-type-pratt-parser` compatible). |
| `typeParseMode` | `'jsdoc' \| 'closure' \| 'typescript'` | `'jsdoc'` | Syntax flavor for type expressions when `parseTypes` is on. |
| `compatMode` | `boolean` | `false` | Emit `@es-joy/jsdoccomment` compatible fields (`delimiter`, `postDelimiter`, `initial`, line indices, …) and exclude ox-jsdoc-specific fields. |
| `emptyStringForNull` | `boolean` | `false` | Convert absent optional strings (`rawType`, `name`, `namepathOrURL`, `text`) to `""` instead of `null`. Mirrors jsdoccomment serialization. |
| `includePositions` | `boolean` | `true` | Include ESTree position fields (`start`, `end`, `range`) on every node. |
| `spacing` | `'compact' \| 'preserve'` | `'compact'` | Spacing mode for compat output. `compact` drops empty description lines like jsdoccomment; `preserve` keeps every scanned line verbatim. `compatMode` only. |

### `compatMode` example

```js
import { parse } from 'ox-jsdoc'

const result = parse('/**\n * @param {string} id\n */', {
  compatMode: true,
  emptyStringForNull: true
})
// result.ast.tags[0] now carries `delimiter`, `postDelimiter`, `postTag`,
// `postType`, `postName`, `initial`, `lineEnd` etc. — ready to feed into
// downstream consumers that expect the @es-joy/jsdoccomment AST shape.
```

## When to use which package

| Need | Use |
| --- | --- |
| Plain JSON AST, immediate access to all fields | **`ox-jsdoc`** (this package) |
| Lazy access, lowest allocation, batch parsing | [`ox-jsdoc-binary`](../ox-jsdoc-binary/) (Binary AST + decoder) |
| `@es-joy/jsdoccomment` compatible AST shape | `@ox-jsdoc/jsdoccomment` (workspace package, builds on `ox-jsdoc-binary`) |

## Build from source

This package is built with [`@napi-rs/cli`](https://napi.rs/):

```sh
pnpm build       # release build
pnpm build-dev   # debug build (faster compile, slower runtime)
pnpm test        # run vitest tests
```

<!-- prettier-ignore -->
> [!NOTE]
> The compiled native module is dropped into `src-js/<binary>.node` and is not checked in.

## License

[MIT](https://opensource.org/licenses/MIT)
