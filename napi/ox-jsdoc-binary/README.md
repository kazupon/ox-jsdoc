# ox-jsdoc-binary

> [!WARNING] **Deprecated alias.** This package is now a JS-only thin re-export of the canonical [`ox-jsdoc`](../ox-jsdoc/) package. Install and import `ox-jsdoc` directly. This alias is published for one deprecation cycle to ease migration from the previously-published `ox-jsdoc-binary` `0.0.12` and will be removed in a subsequent release.

Pre-cutover, `ox-jsdoc-binary` was the Binary AST NAPI binding while `ox-jsdoc` was the typed AST + JSON binding. After the [Binary AST mainstream migration](../../design/010-main-stream-binary/README.md), the Binary AST implementation became the canonical `ox-jsdoc` package, and this package was downgraded to a JS-only re-export so that existing `0.0.12` consumers keep working unchanged for one release cycle.

## Migration

Replace the dependency name:

```diff
- "ox-jsdoc-binary": "^0.0.13"
+ "ox-jsdoc": "^0.0.13"
```

…and the import:

```diff
- import { parse, parseBatch } from 'ox-jsdoc-binary'
+ import { parse, parseBatch } from 'ox-jsdoc'
```

The runtime behavior of the new `ox-jsdoc` matches what `ox-jsdoc-binary` `0.0.12` exported.

## How it works

This package's `src-js/index.js` is a single line:

```js
export * from 'ox-jsdoc'
```

There is no separate native binding, no `napi pre-publish`, and no `@ox-jsdoc/binary-binding-*` platform packages — the canonical `ox-jsdoc` package provides all of them.

## License

[MIT](https://opensource.org/licenses/MIT)
