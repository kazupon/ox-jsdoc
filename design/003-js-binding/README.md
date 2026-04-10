# NAPI / JS Binding (`napi/ox-jsdoc`)

## Context

The Rust core parser, validator, analyzer, and serializer are working.
The next step is to create NAPI bindings so JS/Node.js consumers can use
`ox-jsdoc`.

Design document `002-project-structure` specifies placing the binding crate
under `napi/ox-jsdoc/` and exposing a JS API through JSON-first transfer.

## Goals

- Create a NAPI binding crate + npm package under `napi/ox-jsdoc/`
- Expose a synchronous `parse(sourceText, options?)` JS API
- Use serde_json-based JSON transfer via the existing `serialize_comment_json`
- Auto-generate TypeScript type definitions
- Make build and test runnable through `vp run`

## Files

### New files

| File                               | Description                                     |
| ---------------------------------- | ----------------------------------------------- |
| `napi/ox-jsdoc/Cargo.toml`         | NAPI crate definition                           |
| `napi/ox-jsdoc/build.rs`           | `napi_build::setup()`                           |
| `napi/ox-jsdoc/src/lib.rs`         | `#[napi] parse` function                        |
| `napi/ox-jsdoc/package.json`       | npm package definition (name: `ox-jsdoc`)       |
| `napi/ox-jsdoc/src-js/index.js`    | JS entry point (bindings re-export + lazy wrap) |
| `napi/ox-jsdoc/src-js/index.d.ts`  | TypeScript type definitions                     |
| `napi/ox-jsdoc/test/parse.test.ts` | vitest tests for the `parse` function           |

### Modified files

| File                          | Description             |
| ----------------------------- | ----------------------- |
| `Cargo.toml` (workspace root) | Add `napi/*` to members |

`pnpm-workspace.yaml` already includes `napi/*` — no change needed.

## JS API

```ts
export interface ParseOptions {
  /** Suppress tag recognition inside fenced code blocks. Default: true. */
  fenceAware?: boolean
}

export interface ParseResult {
  /** Parsed JSDoc AST as a JSON object (ESTree-like shape). */
  ast: JsdocBlock
  /** Parser diagnostics. Empty array on successful parse. */
  diagnostics: Diagnostic[]
}

export interface Diagnostic {
  message: string
}

/**
 * Parse a complete `/** ... *​/` JSDoc block comment.
 */
export function parse(sourceText: string, options?: ParseOptions): ParseResult
```

v1 provides only a synchronous function. An async variant may be added later.

## Rust NAPI Function

```rust
#[napi]
pub fn parse(source_text: String, options: Option<JsParseOptions>) -> JsParseResult {
    let allocator = Allocator::default();
    let opts = convert_options(options);
    let output = ox_jsdoc::parse_comment(&allocator, &source_text, 0, opts);

    let (ast_json, diagnostics) = match output.comment {
        Some(comment) => {
            let json = ox_jsdoc::serialize_comment_json(&comment, None, None);
            let diags = convert_diagnostics(&output.diagnostics);
            (json, diags)
        }
        None => {
            let diags = convert_diagnostics(&output.diagnostics);
            (String::from("null"), diags)
        }
    };

    JsParseResult { ast_json, diagnostics }
}
```

## JS Wrapping

Following the oxc pattern, the JSON string is converted to a JS object via a lazy getter:

```js
// src-js/index.js
import { parse as parseBinding } from './bindings.js'

export function parse(sourceText, options) {
  const result = parseBinding(sourceText, options ?? {})
  return {
    get ast() {
      const value = JSON.parse(result.astJson)
      Object.defineProperty(this, 'ast', { value })
      return value
    },
    diagnostics: result.diagnostics
  }
}
```

## Dependencies

### Rust (napi/ox-jsdoc/Cargo.toml)

```toml
[dependencies]
ox_jsdoc = { path = "../../crates/ox_jsdoc" }
oxc_allocator = { workspace = true }
napi = { version = "3", default-features = false }
napi-derive = "3"

[build-dependencies]
napi-build = "2"
```

### Workspace root (Cargo.toml)

```toml
members = [
  "crates/*",
  "tasks/benchmark",
  "tasks/xtask",
  "napi/*",
]
```

### npm (napi/ox-jsdoc/package.json)

```json
{
  "name": "ox-jsdoc",
  "version": "0.0.0",
  "type": "module",
  "main": "src-js/index.js",
  "types": "src-js/index.d.ts",
  "private": true,
  "napi": {
    "binaryName": "ox_jsdoc",
    "packageName": "@ox-jsdoc/binding",
    "targets": [
      "aarch64-apple-darwin",
      "x86_64-apple-darwin",
      "x86_64-unknown-linux-gnu",
      "x86_64-pc-windows-msvc"
    ]
  },
  "scripts": {
    "build": "napi build --esm --platform --js bindings.js --dts bindings.d.ts --output-dir src-js",
    "build:release": "pnpm run build --release",
    "test": "vp test run"
  },
  "devDependencies": {
    "@napi-rs/cli": "^3.6.1",
    "vite-plus": "catalog:"
  }
}
```

## Implementation Steps

1. Create directories: `napi/ox-jsdoc/src/`, `napi/ox-jsdoc/src-js/`
2. Create `Cargo.toml` for the NAPI crate
3. Create `build.rs` with `napi_build::setup()`
4. Implement `src/lib.rs` with `parse` function (`#[napi]`) + option/result types
5. Create `package.json` for the npm package
6. Create `src-js/index.js` — bindings wrapper with lazy JSON parse
7. Create `src-js/index.d.ts` — TypeScript type definitions
8. Update workspace `Cargo.toml` to add `napi/*` to members
9. Build check: `cd napi/ox-jsdoc && pnpm run build`
10. Add vitest tests in `napi/ox-jsdoc/test/parse.test.ts`
11. Run tests: `cd napi/ox-jsdoc && pnpm run test`

## Testing

JS binding tests use vitest.
Test files are placed under `napi/ox-jsdoc/test/`.

### What to test

- `parse()` converts a well-formed JSDoc block into an AST
- The return value has `{ ast, diagnostics }` shape
- `ast.type` equals `"JsdocBlock"`
- Tag fields `tag`, `rawType`, `name`, `description` are correctly extracted
- Malformed input produces `diagnostics`
- The `fenceAware` option works

### Test file

```ts
// napi/ox-jsdoc/test/parse.test.ts
import { describe, expect, it } from 'vite-plus/test'
import { parse } from '../src-js/index.js'

describe('parse', () => {
  it('parses a basic param tag', () => {
    const result = parse('/** @param {string} id - The user ID */')
    expect(result.diagnostics).toEqual([])
    expect(result.ast.type).toBe('JsdocBlock')
    expect(result.ast.tags[0].tag).toBe('param')
    expect(result.ast.tags[0].rawType).toBe('string')
    expect(result.ast.tags[0].name).toBe('id')
    expect(result.ast.tags[0].description).toBe('The user ID')
  })

  it('returns diagnostics for malformed input', () => {
    const result = parse('/** {@link Foo */')
    expect(result.diagnostics.length).toBeGreaterThan(0)
  })

  it('rejects non-jsdoc input', () => {
    const result = parse('/* plain */')
    expect(result.ast).toBeNull()
    expect(result.diagnostics.length).toBeGreaterThan(0)
  })
})
```

### Running

```sh
# Build + test
cd napi/ox-jsdoc
pnpm run build
pnpm run test

# From workspace root
vpr test
```

## CI / CD: GitHub Actions Publish Workflow

### Overview

npm publishing is done through GitHub Actions, not locally.
A matrix build compiles binaries for each platform, then `@napi-rs/cli`
commands generate and publish platform-specific packages.

The workflow follows the oxc `reusable_release.yml` pattern.

### Trigger

```yaml
on:
  push:
    branches: [main]
    paths:
      - napi/ox-jsdoc/package.json
      - .github/workflows/release.yml
```

Auto-publishes when the `version` field in `napi/ox-jsdoc/package.json`
changes on the main branch.

### Workflow Structure

```
.github/workflows/release.yml
  ├─ check
  │   └─ Compare version against npm registry
  │       → Skip remaining jobs if unchanged
  │
  ├─ build (matrix)
  │   ├─ Build binary for each target
  │   └─ Upload .node file as artifact
  │
  └─ publish (needs: build)
      ├─ Download all artifacts
      ├─ napi create-npm-dirs
      ├─ napi artifacts
      ├─ napi pre-publish
      └─ pnpm publish --provenance --access public
```

### Build Matrix

v1 targets four primary platforms. More can be added on demand.

| Target                     | Runner           | Notes                                 |
| -------------------------- | ---------------- | ------------------------------------- |
| `aarch64-apple-darwin`     | `macos-latest`   | Apple Silicon                         |
| `x86_64-apple-darwin`      | `macos-latest`   | Intel Mac                             |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest`  | Linux x64 (glibc), `--use-napi-cross` |
| `x86_64-pc-windows-msvc`   | `windows-latest` | Windows x64                           |

Future candidates:

- `aarch64-unknown-linux-gnu` (ARM Linux)
- `x86_64-unknown-linux-musl` (Alpine / musl)
- `wasm32-wasip1-threads` (Browser / WASI)

### Build Job

```yaml
build:
  if: needs.check.outputs.version_changed == 'true'
  strategy:
    fail-fast: false
    matrix:
      include:
        - os: macos-latest
          target: aarch64-apple-darwin
        - os: macos-latest
          target: x86_64-apple-darwin
        - os: ubuntu-latest
          target: x86_64-unknown-linux-gnu
          use-cross: true
        - os: windows-latest
          target: x86_64-pc-windows-msvc
  runs-on: ${{ matrix.os }}
  steps:
    - name: Checkout codes
      uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2
      with:
        fetch-depth: 0

    - name: Setup Vite Plus
      uses: voidzero-dev/setup-vp@8ecb39174989ce55af90f45cf55b02738599831d # v1.6.0
      with:
        node-version: 24
        cache: true
        run-install: false

    - name: Install dependencies
      run: vp install

    - name: Install Rust toolchain
      run: rustup target add ${{ matrix.target }}

    - name: Build
      working-directory: napi/ox-jsdoc
      run: |
        pnpm build --release --target ${{ matrix.target }} \
          ${{ matrix.use-cross && '--use-napi-cross' || '' }}

    - name: Upload artifact
      uses: actions/upload-artifact@bbbca2ddaa5d8feaa63e36b76fdaad77386f024f # v7.0.0
      with:
        name: binding-${{ matrix.target }}
        path: napi/ox-jsdoc/src-js/*.node
```

### Publish Job

```yaml
publish:
  needs: build
  runs-on: ubuntu-latest
  permissions:
    id-token: write # OIDC trusted publishing
  steps:
    - name: Checkout codes
      uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2
      with:
        fetch-depth: 0

    - name: Setup Vite Plus
      uses: voidzero-dev/setup-vp@8ecb39174989ce55af90f45cf55b02738599831d # v1.6.0
      with:
        node-version: 24
        cache: true
        run-install: false
        registry-url: https://registry.npmjs.org

    - name: Install dependencies
      run: vp install

    - name: Download all artifacts
      uses: actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c # v8.0.1
      with:
        path: artifacts
        merge-multiple: true

    - name: Create npm dirs
      working-directory: napi/ox-jsdoc
      run: pnpm napi create-npm-dirs

    - name: Move artifacts
      working-directory: napi/ox-jsdoc
      run: pnpm napi artifacts --build-output-dir ../../artifacts --npm-dir npm

    - name: Publish platform packages
      working-directory: napi/ox-jsdoc
      run: pnpm napi pre-publish --no-gh-release -t npm
      env:
        NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}

    - name: Publish main package
      working-directory: napi/ox-jsdoc
      run: pnpm publish --provenance --access public --no-git-checks
      env:
        NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

### Published npm Packages

When a user runs `npm install ox-jsdoc`, the following packages are installed:

```
ox-jsdoc                              ← Main package (JS loader + type definitions)
@ox-jsdoc/binding-darwin-arm64        ← macOS ARM64 .node binary
@ox-jsdoc/binding-darwin-x64          ← macOS Intel .node binary
@ox-jsdoc/binding-linux-x64-gnu       ← Linux x64 .node binary
@ox-jsdoc/binding-win32-x64-msvc      ← Windows x64 .node binary
```

The main package declares `optionalDependencies`:

```json
{
  "optionalDependencies": {
    "@ox-jsdoc/binding-darwin-arm64": "0.0.0",
    "@ox-jsdoc/binding-darwin-x64": "0.0.0",
    "@ox-jsdoc/binding-linux-x64-gnu": "0.0.0",
    "@ox-jsdoc/binding-win32-x64-msvc": "0.0.0"
  }
}
```

npm installs only the `optionalDependencies` entry that matches the user's
platform.

### Prerequisites

- Reserve the `ox-jsdoc` package name on npm
- Create the `@ox-jsdoc` scope on npm
- Add `NPM_TOKEN` to GitHub repository secrets
- Enable npm trusted publishing (OIDC) or use `NPM_TOKEN` authentication

### Non-goals for v1

- WASM build (browser support)
- FreeBSD / Android / OpenHarmony targets
- Automatic GitHub Release creation
- Automatic changelog generation
