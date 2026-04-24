/**
 * Phase 2 benchmark — compare parseBatch (single shared buffer) vs N
 * sequential parse() calls. Also includes comment-parser / jsdoccomment
 * baselines for context.
 *
 * Uses `lib/measure.mjs` for variance-resistant aggregation; see the NAPI
 * bench script for the rationale.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

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

import { compareRobust, fmtDuration } from './lib/measure.mjs'

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

const groups = [
  {
    title: 'Batch 100',
    n: 100,
    benches: [
      ['comment-parser (loop)', () => { for (const c of batch100) void commentParserParse(c) }],
      ['jsdoccomment (loop)', () => {
        for (const c of batch100) {
          try { void jsdoccommentParse(c) } catch {}
        }
      }],
      ['ox-jsdoc typed NAPI (loop)', () => { for (const c of batch100) void parseTypedNapi(c).ast }],
      ['ox-jsdoc-binary NAPI (loop)', () => { for (const c of batch100) void parseBinaryNapi(c).ast }],
      ['ox-jsdoc-binary NAPI (parseBatch)', () => { void parseBatchBinaryNapi(batch100Items).asts }],
      ['ox-jsdoc-binary WASM (loop)', () => {
        for (const c of batch100) {
          const r = parseBinaryWasm(c)
          void r.ast
          r.free()
        }
      }],
      ['ox-jsdoc-binary WASM (parseBatch)', () => {
        const r = parseBatchBinaryWasm(batch100Items)
        void r.asts
        r.free()
      }]
    ]
  },
  {
    title: `Full file (${allComments.length} comments)`,
    n: allComments.length,
    benches: [
      ['comment-parser (loop)', () => { for (const c of allComments) void commentParserParse(c) }],
      ['jsdoccomment (loop)', () => {
        for (const c of allComments) {
          try { void jsdoccommentParse(c) } catch {}
        }
      }],
      ['ox-jsdoc typed NAPI (loop)', () => { for (const c of allComments) void parseTypedNapi(c).ast }],
      ['ox-jsdoc-binary NAPI (loop)', () => { for (const c of allComments) void parseBinaryNapi(c).ast }],
      ['ox-jsdoc-binary NAPI (parseBatch)', () => { void parseBatchBinaryNapi(allItems).asts }],
      ['ox-jsdoc-binary WASM (loop)', () => {
        for (const c of allComments) {
          const r = parseBinaryWasm(c)
          void r.ast
          r.free()
        }
      }],
      ['ox-jsdoc-binary WASM (parseBatch)', () => {
        const r = parseBatchBinaryWasm(allItems)
        void r.asts
        r.free()
      }]
    ]
  }
]

for (const g of groups) {
  const benches = g.benches.map(([name, fn]) => ({ name: `${g.title} | ${name}`, fn }))
  const results = await compareRobust(benches)
  printGroup(g.title, results, g.n, `${g.title} | ox-jsdoc-binary NAPI (parseBatch)`)
  console.log('')
}

// Size comparison via parseBatch — string dedup engaged.
const single = parseBatchBinaryNapi([
  { sourceText: '/**\n * @param {string} id\n */', baseOffset: 0 }
])
const singleSize = single.sourceFile.view.byteLength
const dup50 = parseBatchBinaryNapi(
  Array.from({ length: 50 }, () => ({ sourceText: '/**\n * @param {string} id\n */', baseOffset: 0 }))
)
const dup50Size = dup50.sourceFile.view.byteLength

console.log('### String dedup effect (50x identical comments)')
console.log('')
console.log('| Mode | Bytes | Per item |')
console.log('|---|---:|---:|')
console.log(`| 50x parse() | ${(singleSize * 50).toLocaleString()} | ${singleSize.toLocaleString()} |`)
console.log(`| 1x parseBatch x50 | ${dup50Size.toLocaleString()} | ${(dup50Size / 50).toFixed(1)} |`)
console.log(`| Reduction | ${((1 - dup50Size / (singleSize * 50)) * 100).toFixed(1)}% smaller | |`)

function printGroup(title, results, n, referenceName) {
  console.log(`### ${title} — sorted (fastest first)`)
  console.log('')
  console.log('| Parser | Total (spread) | Per comment | vs parseBatch |')
  console.log('|---|---:|---:|---:|')
  const refRow = results.find(r => r.name === referenceName) ?? results[0]
  const sorted = [...results].sort((a, b) => a.p50 - b.p50)
  for (const row of sorted) {
    const display = row.name.includes(' | ') ? row.name.split(' | ').slice(1).join(' | ') : row.name
    const total = `${fmtDuration(row.p50)} (±${row.spread_pct.toFixed(1)}%)`
    const per = fmtDuration(row.p50 / n)
    const ratio = (row.p50 / refRow.p50).toFixed(2)
    console.log(`| ${display} | ${total} | ${per} | **${ratio}x** |`)
  }
}

function extractJsdocComments(filePath, source) {
  const result = parseSync(filePath, source)
  return result.comments
    .filter(c => c.type === 'Block' && c.value.startsWith('*'))
    .map(c => source.slice(c.start, c.end))
}
