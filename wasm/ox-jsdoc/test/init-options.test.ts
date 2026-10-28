// The object form that wasm-bindgen asks for (`{ module_or_path }`) keeps
// working, for callers that switched to it because of the warning.

import { describe, expect, it, vi } from 'vite-plus/test'

import { initWasm, parse } from '../src-js/index.js'

const wasmUrl = new URL('../pkg/ox_jsdoc_wasm_bg.wasm', import.meta.url)

describe('initWasm({ module_or_path })', () => {
  it('accepts the wasm-bindgen object form without a warning', async () => {
    const warn = vi.spyOn(console, 'warn')

    await initWasm({ module_or_path: wasmUrl })

    expect(warn.mock.calls.flat().join('\n')).not.toMatch(/deprecated parameters/)
    const result = parse('/** @param {string} id - The user ID */')
    try {
      expect(result.ast?.tags.length).toBe(1)
    } finally {
      result.free()
    }
  })
})
