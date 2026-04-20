/**
 * Cross-parser benchmark — compares all four JSDoc parsers on the same
 * `typescript-checker.ts` fixture used by the binary AST benches.
 *
 * Parsers:
 * - comment-parser (JS)
 * - @es-joy/jsdoccomment (JS, wraps comment-parser)
 * - ox-jsdoc (NAPI, typed AST + JSON.parse)
 * - ox-jsdoc-binary (NAPI, binary AST + lazy decoder)
 * - @ox-jsdoc/wasm (typed AST via WASM)
 * - @ox-jsdoc/wasm-binary (binary AST via WASM)
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { bench, group, run } from 'mitata'
import { parseSync } from 'oxc-parser'
import { parse as commentParserParse } from 'comment-parser'
import { parseComment as jsdoccommentParse } from '@es-joy/jsdoccomment'
import { parse as parseTypedNapi } from 'ox-jsdoc'
import { parse as parseBinaryNapi } from 'ox-jsdoc-binary'
import { initWasm as initTypedWasm, parse as parseTypedWasm } from '@ox-jsdoc/wasm'
import { initWasm as initBinaryWasm, parse as parseBinaryWasm } from '@ox-jsdoc/wasm-binary'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturePath = path.join(repoRoot, 'fixtures/perf/source/typescript-checker.ts')

await initTypedWasm(
  await readFile(path.join(repoRoot, 'wasm/ox-jsdoc/pkg/ox_jsdoc_wasm_bg.wasm'))
)
await initBinaryWasm(
  await readFile(path.join(repoRoot, 'wasm/ox-jsdoc-binary/pkg/ox_jsdoc_binary_wasm_bg.wasm'))
)

const sourceText = await readFile(fixturePath, 'utf8')
const allComments = extractJsdocComments(fixturePath, sourceText)
const batch100 = allComments.slice(0, 100)

const lengths = allComments.map(c => c.length).sort((a, b) => a - b)
const targetLen = lengths[Math.floor(lengths.length / 2)]
let single = allComments[0]
let bestDelta = Math.abs(allComments[0].length - targetLen)
for (const c of allComments) {
  const d = Math.abs(c.length - targetLen)
  if (d < bestDelta) {
    single = c
    bestDelta = d
  }
}

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts`)
console.log(`Single comment: ${single.length} bytes (median; range ${lengths[0]}-${lengths[lengths.length - 1]})`)
console.log('')

// All ox-jsdoc paths force `.ast` access for fair comparison with typed
// (whose ast getter triggers JSON.parse). comment-parser/jsdoccomment
// always return materialised objects, so no extra access needed.

group('Single comment (median ~207 bytes)', () => {
  bench('comment-parser', () => {
    void commentParserParse(single)
  })
  bench('jsdoccomment', () => {
    try {
      void jsdoccommentParse(single)
    } catch {}
  })
  bench('ox-jsdoc (NAPI typed)', () => {
    void parseTypedNapi(single).ast
  })
  bench('ox-jsdoc-binary (NAPI)', () => {
    void parseBinaryNapi(single).ast
  })
  bench('ox-jsdoc (WASM typed)', () => {
    void parseTypedWasm(single).ast
  })
  bench('ox-jsdoc-binary (WASM)', () => {
    const r = parseBinaryWasm(single)
    void r.ast
    r.free()
  })
})

group('Batch 100 comments', () => {
  bench('comment-parser x100', () => {
    for (const c of batch100) void commentParserParse(c)
  })
  bench('jsdoccomment x100', () => {
    for (const c of batch100) {
      try {
        void jsdoccommentParse(c)
      } catch {}
    }
  })
  bench('ox-jsdoc (NAPI typed) x100', () => {
    for (const c of batch100) void parseTypedNapi(c).ast
  })
  bench('ox-jsdoc-binary (NAPI) x100', () => {
    for (const c of batch100) void parseBinaryNapi(c).ast
  })
  bench('ox-jsdoc (WASM typed) x100', () => {
    for (const c of batch100) void parseTypedWasm(c).ast
  })
  bench('ox-jsdoc-binary (WASM) x100', () => {
    for (const c of batch100) {
      const r = parseBinaryWasm(c)
      void r.ast
      r.free()
    }
  })
})

group(`Full file (${allComments.length} comments)`, () => {
  bench('comment-parser full', () => {
    for (const c of allComments) void commentParserParse(c)
  })
  bench('jsdoccomment full', () => {
    for (const c of allComments) {
      try {
        void jsdoccommentParse(c)
      } catch {}
    }
  })
  bench('ox-jsdoc (NAPI typed) full', () => {
    for (const c of allComments) void parseTypedNapi(c).ast
  })
  bench('ox-jsdoc-binary (NAPI) full', () => {
    for (const c of allComments) void parseBinaryNapi(c).ast
  })
  bench('ox-jsdoc (WASM typed) full', () => {
    for (const c of allComments) void parseTypedWasm(c).ast
  })
  bench('ox-jsdoc-binary (WASM) full', () => {
    for (const c of allComments) {
      const r = parseBinaryWasm(c)
      void r.ast
      r.free()
    }
  })
})

const result = await run({ format: 'quiet', print: () => {}, colors: false, throw: true })
const rows = result.benchmarks.flatMap(b => b.runs.map(r => ({ name: r.name, avgNs: r.stats.avg })))

console.log('')
console.log('| Parser | Single | Batch 100 | Full file |')
console.log('|---|---:|---:|---:|')

const parsers = [
  ['comment-parser', 'comment-parser', 'comment-parser x100', 'comment-parser full'],
  ['jsdoccomment', 'jsdoccomment', 'jsdoccomment x100', 'jsdoccomment full'],
  ['ox-jsdoc (NAPI typed)', 'ox-jsdoc (NAPI typed)', 'ox-jsdoc (NAPI typed) x100', 'ox-jsdoc (NAPI typed) full'],
  ['ox-jsdoc-binary (NAPI)', 'ox-jsdoc-binary (NAPI)', 'ox-jsdoc-binary (NAPI) x100', 'ox-jsdoc-binary (NAPI) full'],
  ['ox-jsdoc (WASM typed)', 'ox-jsdoc (WASM typed)', 'ox-jsdoc (WASM typed) x100', 'ox-jsdoc (WASM typed) full'],
  ['ox-jsdoc-binary (WASM)', 'ox-jsdoc-binary (WASM)', 'ox-jsdoc-binary (WASM) x100', 'ox-jsdoc-binary (WASM) full']
]

for (const [label, single, batch, full] of parsers) {
  const s = rows.find(r => r.name === single)
  const b = rows.find(r => r.name === batch)
  const f = rows.find(r => r.name === full)
  if (!s || !b || !f) continue
  console.log(`| ${label} | ${fmt(s.avgNs)} | ${fmt(b.avgNs)} | ${fmt(f.avgNs)} |`)
}

// Speedup table: ox-jsdoc-binary NAPI vs each baseline
console.log('')
console.log('| Parser | Single (vs binary NAPI) | Full file (vs binary NAPI) |')
console.log('|---|---:|---:|')
const binaryNapiSingle = rows.find(r => r.name === 'ox-jsdoc-binary (NAPI)')?.avgNs
const binaryNapiFull = rows.find(r => r.name === 'ox-jsdoc-binary (NAPI) full')?.avgNs
for (const [label, single, _b, full] of parsers) {
  const s = rows.find(r => r.name === single)?.avgNs
  const f = rows.find(r => r.name === full)?.avgNs
  if (s == null || f == null || binaryNapiSingle == null || binaryNapiFull == null) continue
  const sx = (s / binaryNapiSingle).toFixed(2)
  const fx = (f / binaryNapiFull).toFixed(2)
  console.log(`| ${label} | **${sx}x** | **${fx}x** |`)
}

function extractJsdocComments(filePath, source) {
  const result = parseSync(filePath, source)
  return result.comments
    .filter(c => c.type === 'Block' && c.value.startsWith('*'))
    .map(c => source.slice(c.start, c.end))
}

function fmt(v) {
  if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(3)} ms`
  if (v >= 1_000) return `${(v / 1_000).toFixed(3)} µs`
  return `${v.toFixed(3)} ns`
}
