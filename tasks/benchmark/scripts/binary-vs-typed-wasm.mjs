/**
 * Phase 1.2d benchmark — typed AST vs binary AST through the WASM binding.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { bench, group, run } from 'mitata'
import { parseSync } from 'oxc-parser'
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
const single = allComments[0]
const batch100 = allComments.slice(0, 100)

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts`)
console.log('')

group('Single comment (typed vs binary, WASM)', () => {
  bench('parseTypedWasm', () => parseTypedWasm(single))
  bench('parseBinaryWasm', () => {
    const r = parseBinaryWasm(single)
    r.free()
  })
})

group('Batch 100 (typed vs binary, WASM)', () => {
  bench('parseTypedWasm x100', () => {
    for (const c of batch100) parseTypedWasm(c)
  })
  bench('parseBinaryWasm x100', () => {
    for (const c of batch100) {
      const r = parseBinaryWasm(c)
      r.free()
    }
  })
})

group(`Full file (${allComments.length} comments)`, () => {
  bench('parseTypedWasm full', () => {
    for (const c of allComments) parseTypedWasm(c)
  })
  bench('parseBinaryWasm full', () => {
    for (const c of allComments) {
      const r = parseBinaryWasm(c)
      r.free()
    }
  })
})

const result = await run({ format: 'quiet', print: () => {}, colors: false, throw: true })
const rows = result.benchmarks.flatMap(b => b.runs.map(r => ({ name: r.name, avgNs: r.stats.avg })))

console.log('')
console.log('| Scenario | parseTyped (WASM) | parseBinary (WASM) | Speedup |')
console.log('|---|---:|---:|---:|')

const pairs = [
  ['parseTypedWasm', 'parseBinaryWasm', 'Single comment'],
  ['parseTypedWasm x100', 'parseBinaryWasm x100', 'Batch 100'],
  ['parseTypedWasm full', 'parseBinaryWasm full', `Full file (${allComments.length} comments)`]
]
for (const [t, b, label] of pairs) {
  const tr = rows.find(r => r.name === t)
  const br = rows.find(r => r.name === b)
  if (!tr || !br) continue
  console.log(
    `| ${label} | ${fmt(tr.avgNs)} | ${fmt(br.avgNs)} | **${(tr.avgNs / br.avgNs).toFixed(2)}x** |`
  )
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
