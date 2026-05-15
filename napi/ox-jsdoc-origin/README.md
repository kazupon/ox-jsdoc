# ox-jsdoc-origin

The original typed AST + JSON NAPI binding, kept as a benchmark / reference implementation.

<!-- prettier-ignore -->
> [!WARNING]
> **Not for production use.** This package is `"private": true` and is not
> published to the npm registry. It exists only inside this workspace as the
> reference implementation against which the canonical Binary AST package
> [`ox-jsdoc`](../ox-jsdoc/) is benchmarked. New code should depend on
> `ox-jsdoc` instead.
>
> See [`design/010-main-stream-binary/README.md`](../../design/010-main-stream-binary/README.md)
> for the post-cutover migration that moved this implementation aside under
> the `origin` name.

`ox-jsdoc-origin` parses `/** ... */` comment blocks into a plain JSON AST that is ergonomic to consume from JavaScript or TypeScript. The parser core is implemented in Rust (`crates/ox_jsdoc_origin`) and exposed through a NAPI binding, so you get native parsing speed with no JavaScript-side parser code path.

For the canonical lazy Binary AST path used by all production callers, see [`ox-jsdoc`](../ox-jsdoc/).

## Usage (workspace-only)

This package is not published. The examples below assume it is consumed inside this workspace via `pnpm` workspace `protocol:` resolution (e.g. by `tasks/benchmark/`).

### `parse(sourceText, options?)`

Parse a complete `/** ... */` JSDoc block comment.

```js
import { parse } from 'ox-jsdoc-origin'

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
import { parseType } from 'ox-jsdoc-origin'

parseType('string | number') // 'string | number'
parseType('Array<{ id: string }>') // 'Array<{ id: string }>'
parseType('not a type {{') // null
```

### `parseTypeCheck(typeText, mode?)`

Parse a JSDoc type expression and return `true`/`false` for success without the stringification overhead.

```js
import { parseTypeCheck } from 'ox-jsdoc-origin'

parseTypeCheck('string | number') // true
parseTypeCheck('not a type {{') // false
```

`mode` is `'jsdoc' | 'closure' | 'typescript'` (default: `'jsdoc'`).

## Options

`parse(sourceText, options)` accepts:

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `fenceAware` | `boolean` | `true` | Suppress tag recognition inside fenced code blocks (` ``` `). |
| `parseTypes` | `boolean` | `false` | Parse `{...}` type expressions in tags into a structured `parsedType` AST (`jsdoc-type-pratt-parser` compatible). |
| `typeParseMode` | `'jsdoc' \| 'closure' \| 'typescript'` | `'jsdoc'` | Syntax flavor for type expressions when `parseTypes` is on. |
| `compatMode` | `boolean` | `false` | Emit `@es-joy/jsdoccomment` compatible fields (`delimiter`, `postDelimiter`, `initial`, line indices, …) and exclude ox-jsdoc-origin-specific fields. |
| `emptyStringForNull` | `boolean` | `false` | Convert absent optional strings (`rawType`, `name`, `namepathOrURL`, `text`) to `""` instead of `null`. Mirrors jsdoccomment serialization. |
| `includePositions` | `boolean` | `true` | Include ESTree position fields (`start`, `end`, `range`) on every node. |
| `spacing` | `'compact' \| 'preserve'` | `'compact'` | Spacing mode for compat output. `compact` drops empty description lines like jsdoccomment; `preserve` keeps every scanned line verbatim. `compatMode` only. |

## Related packages

| Need | Use |
| --- | --- |
| Canonical Binary AST + batch parsing (production) | [`ox-jsdoc`](../ox-jsdoc/) |
| Original typed AST + JSON (benchmark / reference only) | **`ox-jsdoc-origin`** (this package) |
| `@es-joy/jsdoccomment` compatible AST shape | `@ox-jsdoc/jsdoccomment` (workspace package) |

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
