// A failed `initWasm()` does not stick: the next call starts over.

import { describe, expect, it, vi } from 'vite-plus/test'

import { initWasm, parse } from '../src-js/index.js'

const wasmUrl = new URL('../pkg/ox_jsdoc_wasm_bg.wasm', import.meta.url)

describe('initWasm after a failure', () => {
  it('can be called again with a valid input', async () => {
    const warn = vi.spyOn(console, 'warn')

    // Only the wasm magic number: not a valid module.
    await expect(initWasm(new Uint8Array([0x00, 0x61, 0x73, 0x6d]))).rejects.toThrow(/WebAssembly/)
    await initWasm(wasmUrl)

    expect(warn.mock.calls.flat().join('\n')).not.toMatch(/deprecated parameters/)
    const result = parse('/** @param {string} id - The user ID */')
    try {
      expect(result.ast?.tags.length).toBe(1)
    } finally {
      result.free()
    }
  })
})
