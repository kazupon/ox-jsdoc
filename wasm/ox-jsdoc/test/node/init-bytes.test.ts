// In Node.js the .wasm file is passed as bytes, because the default location
// is a file: URL that fetch cannot read. That must not print wasm-bindgen's
// deprecation warning.

import { readFile } from 'node:fs/promises'
import { describe, expect, it, vi } from 'vite-plus/test'

import { initWasm, parse } from '../../src-js/index.js'

describe('initWasm(bytes) in Node.js', () => {
  it('initializes without the wasm-bindgen deprecation warning', async () => {
    const warn = vi.spyOn(console, 'warn')

    await initWasm(await readFile(new URL('../../pkg/ox_jsdoc_wasm_bg.wasm', import.meta.url)))

    expect(warn.mock.calls.flat().join('\n')).not.toMatch(/deprecated parameters/)
    const result = parse('/** @param {string} id - The user ID */')
    try {
      expect(result.ast?.tags.length).toBe(1)
    } finally {
      result.free()
    }
  })
})
