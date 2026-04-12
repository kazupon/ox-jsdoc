# @ox-jsdoc/wasm

High-performance JSDoc parser for the browser, powered by WebAssembly.

This package provides the same `parse()` API as the native `ox-jsdoc` Node.js binding, but runs in any browser environment via a 58 KB WASM binary.

> [!WARNING]
> This project is still unstable, so don't use in production.

## Installation

```sh
npm install @ox-jsdoc/wasm

# for pnpm
pnpm add @ox-jsdoc/wasm

# for yarn
yarn add @ox-jsdoc/wasm
```

## Usage

```js
import { initWasm, parse } from '@ox-jsdoc/wasm'

// Initialize the WASM module (required once before calling parse)
await initWasm()

// Parse a JSDoc block comment
const { ast, diagnostics } = parse('/** @param {string} id - The user ID */')

console.log(ast.type) // "JsdocBlock"
console.log(ast.tags[0].tag) // "param"
console.log(ast.tags[0].rawType) // "string"
console.log(ast.tags[0].name) // "id"
console.log(ast.tags[0].description) // "The user ID"
console.log(diagnostics) // []
```

## API

### `initWasm(wasmUrl?)`

Initialize the WASM module. Must be called once before `parse()`. Subsequent calls are no-ops.

- `wasmUrl` (optional) - Custom URL or source for the `.wasm` file. When omitted, the default path is used.
- Returns `Promise<void>`

### `parse(sourceText, options?)`

Parse a complete `/** ... */` JSDoc block comment.

- `sourceText` - The full JSDoc block comment string including `/**` and `*/`
- `options.fenceAware` (default: `true`) - Suppress tag recognition inside fenced code blocks

Returns `{ ast, diagnostics }`:

- `ast` - Parsed JSDoc AST (`JsdocBlock`) or `null` on fatal error
- `diagnostics` - Array of `{ message: string }` for recoverable parse issues

## Sponsors

<p align="center">
  <a href="https://cdn.jsdelivr.net/gh/kazupon/sponsors/sponsors.svg">
    <img alt="sponsor" src='https://cdn.jsdelivr.net/gh/kazupon/sponsors/sponsors.svg'/>
  </a>
</p>

## License

[MIT](https://opensource.org/licenses/MIT)
