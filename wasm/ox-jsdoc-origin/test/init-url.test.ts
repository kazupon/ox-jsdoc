// `initWasm()` initializes the module once per page, and each test file runs
// with fresh module state. So each way of calling it has its own file.

import { describe, expect, it, vi } from 'vite-plus/test'

import { initWasm, parse } from '../src-js/index.js'

const wasmUrl = new URL('../pkg/ox_jsdoc_origin_wasm_bg.wasm', import.meta.url)

describe('initWasm(url)', () => {
  it('initializes once without the wasm-bindgen deprecation warning, even when called twice at once', async () => {
    const warn = vi.spyOn(console, 'warn')
    const fetch = vi.spyOn(globalThis, 'fetch')

    await Promise.all([initWasm(wasmUrl), initWasm(wasmUrl)])

    // The glue passes the URL to fetch once per initialization.
    expect(
      fetch.mock.calls.filter(([input]) => input instanceof URL && input.href === wasmUrl.href)
    ).toHaveLength(1)
    expect(warn.mock.calls.flat().join('\n')).not.toMatch(/deprecated parameters/)
    expect(parse('/** @param {string} id - The user ID */').ast?.tags[0].tag).toBe('param')
  })
})
