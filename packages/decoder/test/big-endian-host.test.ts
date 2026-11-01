import { describe, expect, it, vi } from 'vite-plus/test'

import { RemoteSourceFile } from '../src/index.ts'
import type * as Constants from '../src/internal/constants.ts'
import { FixtureBuilder } from './fixture-builder.ts'

// The decoder reads u32 values through a `Uint32Array`, in the byte order of
// the host; the Binary AST is little-endian.
vi.mock('../src/internal/constants.ts', async importOriginal => ({
  ...(await importOriginal<typeof Constants>()),
  HOST_IS_LITTLE_ENDIAN: false
}))

describe('RemoteSourceFile on a big-endian host', () => {
  it('throws instead of reading wrong values', () => {
    expect(() => new RemoteSourceFile(new FixtureBuilder().build())).toThrow(/little-endian/)
  })
})
