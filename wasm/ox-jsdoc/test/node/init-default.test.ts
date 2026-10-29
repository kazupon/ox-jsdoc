// Without an argument, initWasm() fetches the .wasm file next to the glue.
// Node.js cannot fetch that file: URL, so the error says what to pass instead.

import { describe, expect, it } from 'vite-plus/test'

import { initWasm } from '../../src-js/index.js'

describe('initWasm() without an argument in Node.js', () => {
  it('fails with an error that explains how to pass the .wasm file', async () => {
    const initialization = initWasm()

    await expect(initialization).rejects.toThrow(/Pass the \.wasm source to initWasm\(\)/)
    // The original error (`fetch failed`) is kept as the cause.
    await expect(initialization).rejects.toHaveProperty('cause', expect.any(TypeError))
  })
})
