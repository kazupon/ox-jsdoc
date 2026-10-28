# @ox-jsdoc/eslint-plugin-jsdoc

`@ox-jsdoc/eslint-plugin-jsdoc` is a fork of `eslint-plugin-jsdoc` adapted
for `ox-jsdoc`. It follows `eslint-plugin-jsdoc` v65.4.2.

This package reuses the rule implementations from `eslint-plugin-jsdoc` while
exercising `@ox-jsdoc/jsdoccomment` as the JSDoc parser. It is published as part
of the `ox-jsdoc` package family for ESLint / Oxlint integration experiments and
benchmarks.

## Role

- Preserve the rule behavior of `eslint-plugin-jsdoc` as faithfully as
  possible.
- Use `@ox-jsdoc/jsdoccomment` instead of `@es-joy/jsdoccomment`.
- Allow switching between `parseComment` and `parseCommentBatch` from
  `@ox-jsdoc/jsdoccomment` within the same ESLint plugin so the two
  strategies can be compared head-to-head.
- Measure, in `ox-jsdoc` benchmarks, both the performance impact of
  swapping the parser and the benefit of batch parsing.

## Key differences from upstream eslint-plugin-jsdoc

| Item                 | Upstream `eslint-plugin-jsdoc` | This package                                          |
| -------------------- | ------------------------------ | ----------------------------------------------------- |
| package name         | `eslint-plugin-jsdoc`          | `@ox-jsdoc/eslint-plugin-jsdoc`                       |
| publish              | npm package                    | npm package under the `@ox-jsdoc` scope               |
| JSDoc parser         | `@es-joy/jsdoccomment`         | `@ox-jsdoc/jsdoccomment`                              |
| single comment parse | `parseComment`                 | `parseComment` via `@ox-jsdoc/jsdoccomment`           |
| batch parse          | not available                  | enabled via `settings.jsdoc.oxParseStrategy: 'batch'` |
| purpose              | general-purpose ESLint plugin  | fork for ox-jsdoc integration / benchmarking          |

The rule names, configuration, and core rule behavior aim to remain
compatible with upstream `eslint-plugin-jsdoc`. However, since this fork
exists primarily for benchmarking and validation, compatibility follows the
needs of the `ox-jsdoc` integration packages.

## Installation

```sh
npm install @ox-jsdoc/eslint-plugin-jsdoc
```

The package is ESM only and requires Node.js `^22.22.2 || >=24.15.0`.
`require()` also works (through `require(esm)`), and gives `rules` and
`configs` as named exports.

## oxParseStrategy

This fork lets you switch how JSDoc comments are parsed via
`settings.jsdoc.oxParseStrategy`.

```js
import jsdoc from '@ox-jsdoc/eslint-plugin-jsdoc'

export default [
  {
    plugins: {
      jsdoc
    },
    settings: {
      jsdoc: {
        oxParseStrategy: 'single'
      }
    },
    rules: {
      'jsdoc/empty-tags': 'error',
      'jsdoc/require-param-description': 'error',
      'jsdoc/require-param-type': 'error'
    }
  }
]
```

`oxParseStrategy` values:

| Value    | Behavior                                                                                                        |
| -------- | --------------------------------------------------------------------------------------------------------------- |
| `single` | Calls `parseComment` from `@ox-jsdoc/jsdoccomment` once per comment. Default.                                   |
| `batch`  | Collects JSDoc comments from `SourceCode#getAllComments()` and parses them all at once via `parseCommentBatch`. |

`batch` parses the JSDoc comments of a file once and caches the result per
`SourceCode`. Each rule gets its own copy of a parsed comment, so a rule (or
its fixer) that changes the comment does not affect the other rules. See also
[`oxParseStrategy` in the settings](./docs/settings.md#settings-oxparsestrategy).

## Oxlint

The plugin also runs as an Oxlint JS plugin. `jsdoc` is the name of Oxlint's
built-in JSDoc plugin, so give the JS plugin another name:

```json
{
  "jsPlugins": [{ "name": "jsdoc-js", "specifier": "@ox-jsdoc/eslint-plugin-jsdoc" }],
  "settings": {
    "jsdoc": {
      "oxParseStrategy": "batch"
    }
  },
  "rules": {
    "jsdoc-js/empty-tags": "error"
  }
}
```

The `ox-jsdoc` repository checks in CI that ESLint and Oxlint report the same
diagnostics for a set of rules (`jsdoc-linter:parity`). Some features need
ESLint:

- `no-unnecessary-type-assertion` needs the type information of
  `typescript-eslint`, which Oxlint JS plugins do not get.
- Processors (`getJsdocProcessorPlugin`, used by the `examples` configs) are
  not supported by Oxlint.

## Intended benchmark scenarios

This package is meant to support comparisons such as:

- `eslint + eslint-plugin-jsdoc (@es-joy/jsdoccomment)`
- `eslint + @ox-jsdoc/eslint-plugin-jsdoc (oxParseStrategy: 'single')`
- `eslint + @ox-jsdoc/eslint-plugin-jsdoc (oxParseStrategy: 'batch')`
- `oxlint + @ox-jsdoc/eslint-plugin-jsdoc (oxParseStrategy: 'batch')`

## Credit

This package is based on
[`eslint-plugin-jsdoc`](https://github.com/gajus/eslint-plugin-jsdoc), created by
Gajus Kuizinas and maintained by its contributors.

The original project provides the ESLint rule implementation, configuration
structure, tests, and documentation that this fork is derived from.

`@ox-jsdoc/eslint-plugin-jsdoc` contains modifications for the `ox-jsdoc`
project, including parser replacement with `@ox-jsdoc/jsdoccomment` and
`oxParseStrategy` support.

## License

This package is distributed under the BSD-3-Clause license, matching the
upstream `eslint-plugin-jsdoc` license.

See `LICENSE` for the full license text.

Original code copyright:

- Copyright (c) 2018, Gajus Kuizinas

Modifications in this repository are part of the `ox-jsdoc` project and are
distributed under the same BSD-3-Clause license unless otherwise noted.
