/**
 * Smoke test for the JS-only `@ox-jsdoc/wasm-binary` alias package. Verifies
 * that the alias re-exports the canonical `@ox-jsdoc/wasm` surface.
 *
 * `initWasm()` is not exercised here: the canonical WASM package targets
 * `web` and its `initWasm` resolves the `.wasm` via `fetch`, which is not a
 * straight `node` CLI scenario. Functional WASM behavior is covered by
 * `wasm/ox-jsdoc/test/parse.test.ts` against the canonical package directly.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { describe, expect, it } from 'vite-plus/test'
import * as alias from '../src-js/index.js'

describe('@ox-jsdoc/wasm-binary alias', () => {
  it('re-exports the canonical WASM surface', () => {
    const expected = [
      'parse',
      'parseBatch',
      'parseType',
      'parseTypeCheck',
      'initWasm',
      'jsdocVisitorKeys'
    ]
    const actual = Object.keys(alias).sort()
    for (const name of expected) {
      expect(actual.includes(name), `missing export: ${name}`).toBe(true)
    }
  })
})
