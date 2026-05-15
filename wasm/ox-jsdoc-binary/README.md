# @ox-jsdoc/wasm-binary

> [!WARNING] **Deprecated alias.** This package is now a JS-only thin re-export of the canonical [`@ox-jsdoc/wasm`](../ox-jsdoc/) package. Install and import `@ox-jsdoc/wasm` directly. This alias is published for one deprecation cycle to ease migration from the previously-published `@ox-jsdoc/wasm-binary` `0.0.12` and will be removed in a subsequent release.

Pre-cutover, `@ox-jsdoc/wasm-binary` was the Binary AST WASM binding while `@ox-jsdoc/wasm` was the typed AST + JSON binding. After the [Binary AST mainstream migration](../../design/010-main-stream-binary/README.md), the Binary AST implementation became the canonical `@ox-jsdoc/wasm` package, and this package was downgraded to a JS-only re-export so that existing `0.0.12` consumers keep working unchanged for one release cycle.

## Migration

Replace the dependency name:

```diff
- "@ox-jsdoc/wasm-binary": "^0.0.13"
+ "@ox-jsdoc/wasm": "^0.0.13"
```

…and the import:

```diff
- import { initWasm, parse, parseBatch } from '@ox-jsdoc/wasm-binary'
+ import { initWasm, parse, parseBatch } from '@ox-jsdoc/wasm'
```

The runtime behavior of the new `@ox-jsdoc/wasm` matches what `@ox-jsdoc/wasm-binary` `0.0.12` exported.

## How it works

This package's `src-js/index.js` is a single line:

```js
export * from '@ox-jsdoc/wasm'
```

There is no separate WASM artifact — the canonical `@ox-jsdoc/wasm` package provides the `.wasm` binary and the lazy decoder bridge.

## License

[MIT](https://opensource.org/licenses/MIT)
