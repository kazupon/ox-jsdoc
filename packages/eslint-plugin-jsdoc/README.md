# @ox-jsdoc/eslint-plugin-jsdoc

`@ox-jsdoc/eslint-plugin-jsdoc` は、`ox-jsdoc` 向けに fork した
`eslint-plugin-jsdoc` です。

この package は private workspace package であり、npm publish する汎用 package
ではありません。主な目的は、`eslint-plugin-jsdoc` の rule 実装を使いながら、
JSDoc parser として `@ox-jsdoc/jsdoccomment` を試し、ESLint / Oxlint integration
や benchmark を行うことです。

## 役割

- `eslint-plugin-jsdoc` の rule behavior をできるだけ維持する。
- `@es-joy/jsdoccomment` の代わりに `@ox-jsdoc/jsdoccomment` を使う。
- `@ox-jsdoc/jsdoccomment` の `parseComment` と `parseCommentBatch` を、同じ
  ESLint plugin から切り替えて比較できるようにする。
- `ox-jsdoc` の benchmark で、parser 差し替えによる性能差と batch parse の効果を
  計測する。

## 本家 eslint-plugin-jsdoc との主な違い

| 項目 | 本家 `eslint-plugin-jsdoc` | この package |
|---|---|---|
| package name | `eslint-plugin-jsdoc` | `@ox-jsdoc/eslint-plugin-jsdoc` |
| publish | npm package | private workspace package |
| JSDoc parser | `@es-joy/jsdoccomment` | `@ox-jsdoc/jsdoccomment` |
| single comment parse | `parseComment` | `parseComment` via `@ox-jsdoc/jsdoccomment` |
| batch parse | なし | `settings.jsdoc.oxParseStrategy: 'batch'` で利用 |
| 目的 | 一般利用向け ESLint plugin | ox-jsdoc integration / benchmark 用 fork |

rule 名、config、rule の基本挙動は本家 `eslint-plugin-jsdoc` との互換性を保つことを
目標にしています。ただし、この fork は benchmark と検証を主目的としているため、
公開 package としての backward compatibility は保証しません。

## oxParseStrategy

この fork では `settings.jsdoc.oxParseStrategy` により、JSDoc comment の parse 方法を
切り替えられます。

```js
import jsdoc from '@ox-jsdoc/eslint-plugin-jsdoc';

export default [
  {
    plugins: {
      jsdoc,
    },
    settings: {
      jsdoc: {
        oxParseStrategy: 'single',
      },
    },
    rules: {
      'jsdoc/empty-tags': 'error',
      'jsdoc/require-param-description': 'error',
      'jsdoc/require-param-type': 'error',
    },
  },
];
```

`oxParseStrategy` の値:

| 値 | 挙動 |
|---|---|
| `single` | comment ごとに `@ox-jsdoc/jsdoccomment` の `parseComment` を呼ぶ。デフォルト。 |
| `batch` | `SourceCode#getAllComments()` から JSDoc comment を集め、`parseCommentBatch` でまとめて parse する。 |

`batch` は `SourceCode` 単位で parse result を保持し、同一 lint run 内で同じ comment を
再利用します。これにより、複数 rule を同時に有効にした場合でも batch parse の効果を
測りやすくしています。

## Benchmark での想定

この package は、次のような比較に使います。

- `eslint + eslint-plugin-jsdoc (@es-joy/jsdoccomment)`
- `eslint + @ox-jsdoc/eslint-plugin-jsdoc (oxParseStrategy: 'single')`
- `eslint + @ox-jsdoc/eslint-plugin-jsdoc (oxParseStrategy: 'batch')`
- `oxlint + @ox-jsdoc/eslint-plugin-jsdoc (oxParseStrategy: 'batch')`

詳細は repository root の `.notes/jsdoc-linter-benchmark-design.md` を参照してください。

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
