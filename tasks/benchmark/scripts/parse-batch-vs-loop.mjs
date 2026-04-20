/**
 * Phase 2 benchmark — compare parseBatch (single shared buffer) vs N
 * sequential parse() calls. Also includes comment-parser / jsdoccomment
 * baselines for context.
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
import {
  parse as parseBinaryNapi,
  parseBatch as parseBatchBinaryNapi
} from 'ox-jsdoc-binary'
import {
  initWasm as initBinaryWasm,
  parse as parseBinaryWasm,
  parseBatch as parseBatchBinaryWasm
} from '@ox-jsdoc/wasm-binary'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturePath = path.join(repoRoot, 'fixtures/perf/source/typescript-checker.ts')

await initBinaryWasm(
  await readFile(path.join(repoRoot, 'wasm/ox-jsdoc-binary/pkg/ox_jsdoc_binary_wasm_bg.wasm'))
)

const sourceText = await readFile(fixturePath, 'utf8')
const allComments = extractJsdocComments(fixturePath, sourceText)
const batch100 = allComments.slice(0, 100)

const allItems = allComments.map(c => ({ sourceText: c, baseOffset: 0 }))
const batch100Items = batch100.map(c => ({ sourceText: c, baseOffset: 0 }))

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts`)
console.log(`Batch 100 cumulative length: ${batch100.reduce((a, c) => a + c.length, 0)} bytes`)
console.log('')

group('Batch 100', () => {
  bench('B100: comment-parser (loop)', () => {
    for (const c of batch100) void commentParserParse(c)
  })
  bench('B100: jsdoccomment (loop)', () => {
    for (const c of batch100) {
      try {
        void jsdoccommentParse(c)
      } catch {}
    }
  })
  bench('B100: ox-jsdoc typed NAPI (loop)', () => {
    for (const c of batch100) void parseTypedNapi(c).ast
  })
  bench('B100: ox-jsdoc-binary NAPI (loop)', () => {
    for (const c of batch100) void parseBinaryNapi(c).ast
  })
  bench('B100: ox-jsdoc-binary NAPI (parseBatch)', () => {
    void parseBatchBinaryNapi(batch100Items).asts
  })
  bench('B100: ox-jsdoc-binary WASM (loop)', () => {
    for (const c of batch100) {
      const r = parseBinaryWasm(c)
      void r.ast
      r.free()
    }
  })
  bench('B100: ox-jsdoc-binary WASM (parseBatch)', () => {
    const r = parseBatchBinaryWasm(batch100Items)
    void r.asts
    r.free()
  })
})

group('Full file', () => {
  bench('FF: comment-parser (loop)', () => {
    for (const c of allComments) void commentParserParse(c)
  })
  bench('FF: jsdoccomment (loop)', () => {
    for (const c of allComments) {
      try {
        void jsdoccommentParse(c)
      } catch {}
    }
  })
  bench('FF: ox-jsdoc typed NAPI (loop)', () => {
    for (const c of allComments) void parseTypedNapi(c).ast
  })
  bench('FF: ox-jsdoc-binary NAPI (loop)', () => {
    for (const c of allComments) void parseBinaryNapi(c).ast
  })
  bench('FF: ox-jsdoc-binary NAPI (parseBatch)', () => {
    void parseBatchBinaryNapi(allItems).asts
  })
  bench('FF: ox-jsdoc-binary WASM (loop)', () => {
    for (const c of allComments) {
      const r = parseBinaryWasm(c)
      void r.ast
      r.free()
    }
  })
  bench('FF: ox-jsdoc-binary WASM (parseBatch)', () => {
    const r = parseBatchBinaryWasm(allItems)
    void r.asts
    r.free()
  })
})

const result = await run({ format: 'quiet', print: () => {}, colors: false, throw: true })
const rows = result.benchmarks.flatMap(b =>
  b.runs.map(r => ({ name: r.name, avgNs: r.stats.avg }))
)

console.log('')
printGroup(
  'Batch 100',
  rows.filter(r => r.name.startsWith('B100:')).map(r => ({ ...r, name: r.name.slice(6) })),
  100,
  'B100: ox-jsdoc-binary NAPI (parseBatch)'.slice(6)
)
console.log('')
printGroup(
  `Full file (${allComments.length})`,
  rows.filter(r => r.name.startsWith('FF:')).map(r => ({ ...r, name: r.name.slice(4) })),
  allComments.length,
  'FF: ox-jsdoc-binary NAPI (parseBatch)'.slice(4)
)

// Size comparison
const single = parseBatchBinaryNapi([{ sourceText: '/**\n * @param {string} id\n */', baseOffset: 0 }])
const singleSize = single.sourceFile.view.byteLength
const dup50Items = Array.from({ length: 50 }, () => ({ sourceText: '/**\n * @param {string} id\n */', baseOffset: 0 }))
const dup50 = parseBatchBinaryNapi(dup50Items)
const dup50Size = dup50.sourceFile.view.byteLength

console.log('')
console.log('### String dedup effect (50x identical comments)')
console.log('')
console.log(`| Mode | Bytes | Per item |`)
console.log('|---|---:|---:|')
console.log(`| 50x parse() | ${(singleSize * 50).toLocaleString()} | ${singleSize.toLocaleString()} |`)
console.log(`| 1x parseBatch x50 | ${dup50Size.toLocaleString()} | ${(dup50Size / 50).toFixed(1)} |`)
console.log(`| Reduction | ${((1 - dup50Size / (singleSize * 50)) * 100).toFixed(1)}% smaller | |`)

function printGroup(title, groupRows, n, referenceName) {
  console.log(`### ${title} — sorted (fastest first)`)
  console.log('')
  console.log('| Parser | Total | Per comment | vs parseBatch |')
  console.log('|---|---:|---:|---:|')
  const refRow = groupRows.find(r => r.name === referenceName)
  const reference = refRow ? refRow.avgNs : groupRows[0].avgNs
  const sorted = [...groupRows].sort((a, b) => a.avgNs - b.avgNs)
  for (const row of sorted) {
    const total = fmt(row.avgNs)
    const per = fmt(row.avgNs / n)
    const ratio = (row.avgNs / reference).toFixed(2)
    console.log(`| ${row.name} | ${total} | ${per} | **${ratio}x** |`)
  }
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
