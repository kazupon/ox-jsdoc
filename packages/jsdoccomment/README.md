# @ox-jsdoc/jsdoccomment

`@ox-jsdoc/jsdoccomment` is a fork of `@es-joy/jsdoccomment` adapted for
`ox-jsdoc`.

This package is a private workspace package and is not a general-purpose
package intended for npm publish. Its main purpose is to keep the public
runtime surface expected by `eslint-plugin-jsdoc` while replacing the hot
JSDoc parsing path with `ox-jsdoc-binary`.

## Role

- Preserve the `@es-joy/jsdoccomment` API shape needed by
  `eslint-plugin-jsdoc`.
- Use `ox-jsdoc-binary` for JSDoc block parsing instead of
  `comment-parser` tokenization.
- Normalize `ox-jsdoc-binary` output back into the `comment-parser` /
  `jsdoccomment`-compatible block shape consumed by existing rules.
- Expose both single-comment parsing and batch parsing so ESLint / Oxlint
  integration and benchmarks can compare parser strategies.
- Keep upstream runtime helpers such as comment attachment, ESTree conversion,
  stringification, visitor keys, and inline tag parsing available for
  compatibility.

## Key differences from upstream jsdoccomment

| Item         | Upstream `@es-joy/jsdoccomment` | This package                                      |
| ------------ | -------------------------------- | ------------------------------------------------- |
| package name | `@es-joy/jsdoccomment`           | `@ox-jsdoc/jsdoccomment`                          |
| publish      | npm package                      | private workspace package                         |
| parser path  | `comment-parser` tokenizers      | `ox-jsdoc-binary` with compatibility normalization |
| batch parse  | not available                    | `parseCommentBatch`                               |
| primary use  | general-purpose JSDoc utility    | ox-jsdoc integration / compatibility / benchmarking |

The exported AST and helper APIs aim to remain compatible with the upstream
package where `eslint-plugin-jsdoc` depends on them. However, this fork exists
primarily for `ox-jsdoc` development, so it does not guarantee compatibility as
a published package.

## Parser entry points

### `parseComment(commentOrNode, indent?)`

Parses one JSDoc comment string or ESLint comment-token-like object.

Internally, this calls `ox-jsdoc-binary` with jsdoccomment compatibility
options and normalizes the result into a `comment-parser`-compatible block:

- `description`
- `source`
- `problems`
- `tags`
- `inlineTags`

This is the default single-comment path used by
`@ox-jsdoc/eslint-plugin-jsdoc`.

```js
import { parseComment } from '@ox-jsdoc/jsdoccomment'

const block = parseComment('/**\n * @param {string} id\n */')
```

### `parseCommentBatch(comments, options?)`

Parses multiple JSDoc comments with one `ox-jsdoc-binary` batch call and
returns blocks normalized to the same shape as `parseComment`.

```js
import { parseCommentBatch } from '@ox-jsdoc/jsdoccomment'

const { blocks, problems } = parseCommentBatch([
  '/** first */',
  '/**\n * @param {string} id\n */'
])
```

Options:

| Option         | Default | Behavior                                      |
| -------------- | ------- | --------------------------------------------- |
| `indent`       | `''`    | Passed through like the second `parseComment` argument. |
| `throwOnError` | `false` | Throws when any item fails to parse.          |

When an item cannot be parsed as a JSDoc block, its slot in `blocks` is
`null` and `problems` contains indexed diagnostics for that item.

## Other public APIs

This fork continues to export the upstream runtime helpers used by
`eslint-plugin-jsdoc` and compatibility tests:

- `commentParserToESTree` converts the normalized block shape to the
  ESTree/ESLint-friendly JSDoc AST.
- `estreeToString` stringifies the ESTree-style JSDoc AST.
- `getJSDocComment`, `getNonJsdocComment`, and `getReducedASTNode` implement
  ESLint source-code comment attachment behavior.
- `commentHandler` supports selector-based JSDoc AST matching.
- `parseInlineTags` parses inline `{@link ...}` / `{@tutorial ...}` content.
- `jsdocVisitorKeys` and `jsdocTypeVisitorKeys` expose traversal keys.
- `defaultNoTypes`, `defaultNoNames`, `hasSeeWithLink`, and `getTokenizers`
  are retained for compatibility with upstream call sites.
- `jsdoc-type-pratt-parser` exports are re-exported for type parsing and
  stringification support.

## Intended benchmark scenarios

This package is meant to support comparisons such as:

- `@es-joy/jsdoccomment` single-comment parsing
- `@ox-jsdoc/jsdoccomment` single-comment parsing via `parseComment`
- `@ox-jsdoc/jsdoccomment` batch parsing via `parseCommentBatch`
- `@ox-jsdoc/eslint-plugin-jsdoc` with
  `settings.jsdoc.oxParseStrategy: 'single'`
- `@ox-jsdoc/eslint-plugin-jsdoc` with
  `settings.jsdoc.oxParseStrategy: 'batch'`

## Development

Run this package's tests from the repository root:

```sh
pnpm --filter @ox-jsdoc/jsdoccomment test
```

Build the package from the repository root:

```sh
pnpm --filter @ox-jsdoc/jsdoccomment build
```

## Credit

This package is based on
[`@es-joy/jsdoccomment`](https://github.com/es-joy/jsdoccomment), originally
created by Brett Zamir and maintained by its contributors.

The original project provides the public API shape, runtime helpers, tests, and
documentation that this fork is derived from.

`@ox-jsdoc/jsdoccomment` contains modifications for the `ox-jsdoc` project,
including parser replacement with `ox-jsdoc-binary` and the
`parseCommentBatch` API.

## License

This package is distributed under the MIT license, matching the upstream
`@es-joy/jsdoccomment` license.

See `LICENSE-MIT.txt` for the full license text.
