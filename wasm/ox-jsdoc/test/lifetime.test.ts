import type { RemoteJsdocBlock, RemoteJsdocTag, RemoteJsdocTagNameValue } from '@ox-jsdoc/decoder'
import { beforeAll, describe, expect, it } from 'vite-plus/test'

import init from '../pkg/ox_jsdoc_wasm.js'
import { initWasm, parse, parseBatch } from '../src-js/index.js'

// The wasm memory; only its `buffer` is read (the type checking of the tests
// has no DOM lib for `WebAssembly.Memory`).
let memory: { readonly buffer: ArrayBuffer }

beforeAll(async () => {
  await initWasm()
  // Called again, the glue returns the exports that initWasm() set up.
  memory = (await init()).memory
})

const comment = (word: string) =>
  `/**\n * Block summary ${word}.\n * @param {string} ${word} first description\n */`

const read = (ast: RemoteJsdocBlock | null) => ({
  description: ast?.description,
  name: ((ast?.tags[0] as RemoteJsdocTag | undefined)?.name as RemoteJsdocTagNameValue | undefined)
    ?.raw
})

/** Parse a comment larger than wasm memory, so that the memory grows. */
const growMemory = () => {
  const before = memory.buffer.byteLength
  parse(`/** ${'Y'.repeat(before)} */`).free()
  expect(memory.buffer.byteLength).toBeGreaterThan(before)
}

// The AST of `parse` and `parseBatch` must not depend on the wasm-side bytes:
// wasm memory grows and replaces its `ArrayBuffer`, and the bytes of a result
// are freed by `free()`, or once the result object is collected.
describe('the AST stays readable', () => {
  it('after wasm memory grows (parse)', () => {
    const result = parse(comment('keepme'))
    growMemory()
    expect(read(result.ast)).toEqual({ description: 'Block summary keepme.', name: 'keepme' })
    result.free()
  })

  it('after wasm memory grows (parseBatch)', () => {
    const result = parseBatch([{ sourceText: comment('first') }, { sourceText: comment('second') }])
    growMemory()
    expect(result.asts.map(read)).toEqual([
      { description: 'Block summary first.', name: 'first' },
      { description: 'Block summary second.', name: 'second' }
    ])
    result.free()
  })

  it('after free(), when later comments of the same size reuse the memory (parse)', () => {
    const result = parse(comment('keepme'))
    result.free()
    const others = Array.from({ length: 3 }, () => parse(comment('XXXXXX')))
    expect(read(result.ast)).toEqual({ description: 'Block summary keepme.', name: 'keepme' })
    for (const other of others) {
      other.free()
    }
  })

  it('after free(), when later comments of the same size reuse the memory (parseBatch)', () => {
    const result = parseBatch([{ sourceText: comment('first') }, { sourceText: comment('second') }])
    result.free()
    const others = Array.from({ length: 3 }, () =>
      parseBatch([{ sourceText: comment('XXXXX') }, { sourceText: comment('YYYYYY') }])
    )
    expect(result.asts.map(read)).toEqual([
      { description: 'Block summary first.', name: 'first' },
      { description: 'Block summary second.', name: 'second' }
    ])
    for (const other of others) {
      other.free()
    }
  })
})
