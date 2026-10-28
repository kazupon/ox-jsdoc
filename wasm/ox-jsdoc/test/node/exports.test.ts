// The .wasm file is exported, so it can be located from outside the package
// (here through the package's reference to itself by name).

import { createRequire } from 'node:module'
import { describe, expect, it } from 'vite-plus/test'

describe('package exports', () => {
  it('exports the .wasm file as @ox-jsdoc/wasm/ox_jsdoc_wasm_bg.wasm', () => {
    const require = createRequire(import.meta.url)

    expect(require.resolve('@ox-jsdoc/wasm/ox_jsdoc_wasm_bg.wasm')).toMatch(
      /[/\\]pkg[/\\]ox_jsdoc_wasm_bg\.wasm$/
    )
  })
})
