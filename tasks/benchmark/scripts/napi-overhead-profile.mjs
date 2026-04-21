/**
 * Phase-by-phase profile of NAPI parseBatch overhead.
 *
 * Intent: with the JS-side decoder largely optimized, we want to know
 * where the remaining ~250 µs (NAPI parseBatch full file 700 µs vs Rust
 * direct 463 µs) actually goes — pure NAPI call overhead, JS object
 * construction, or the lazy decoder's `asts` materialization.
 *
 * Phases (each inclusive of all earlier phases, so subtraction yields the
 * incremental cost of the new step):
 *
 *   A. `parseJsdocBatchBinding(items, opts)` only — pure NAPI call,
 *      result discarded. Captures rust + binding marshalling.
 *   B. A + `new RemoteSourceFile(r.buffer)` — adds Header parse +
 *      `Uint32Array`/`Map`/`nodeCache` allocs.
 *   C. B + `void sf.asts` — adds N root-class instantiations.
 *   D. Full wrapper `parseBatchBinaryNapi(items)` — adds the `items.map`
 *      remap + result object construction.
 *   E. D + `.asts` access — what the Phase 2 bench actually measures.
 *
 * Plus a pure JS-side baseline (item array remap on its own) to bound
 * how much the wrapper's `.map` contributes.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { parseSync } from 'oxc-parser'
import {
  parseJsdocBatch as napiCall,
  napiMarshallingInOnly,
  napiMarshallingOutOnly
} from 'ox-jsdoc-binary/src-js/bindings.js'
import { parseBatch as parseBatchWrapper } from 'ox-jsdoc-binary'
import { RemoteSourceFile } from '../../../packages/decoder/src/index.js'

import { compareRobust, fmtDuration } from './lib/measure.mjs'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturePath = path.join(repoRoot, 'fixtures/perf/source/typescript-checker.ts')

const sourceText = await readFile(fixturePath, 'utf8')
const allComments = extractJsdocComments(fixturePath, sourceText)
const items = allComments.map(c => ({ sourceText: c, baseOffset: 0 }))
const opts = {}

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts`)
const probeBuffer = napiCall(items, opts).buffer
console.log(`Buffer size for full file: ${probeBuffer.byteLength.toLocaleString()} bytes`)
console.log('')

const benches = [
  {
    name: 'A. NAPI call only (discard result)',
    fn: () => {
      napiCall(items, opts)
    }
  },
  {
    name: 'B. + new RemoteSourceFile(buffer)',
    fn: () => {
      const r = napiCall(items, opts)
      new RemoteSourceFile(r.buffer)
    }
  },
  {
    name: 'C. + void sf.asts',
    fn: () => {
      const r = napiCall(items, opts)
      const sf = new RemoteSourceFile(r.buffer)
      void sf.asts
    }
  },
  {
    name: 'D. Full wrapper parseBatch (no .asts)',
    fn: () => {
      parseBatchWrapper(items, opts)
    }
  },
  {
    name: 'E. Full wrapper + void .asts (bench reference)',
    fn: () => {
      void parseBatchWrapper(items, opts).asts
    }
  },
  {
    name: 'X. items.map remap only (wrapper overhead probe)',
    fn: () => {
      void items.map(it => ({ sourceText: it.sourceText, baseOffset: it.baseOffset }))
    }
  },
  {
    name: 'P. NAPI input marshalling only (no parse, no output)',
    fn: () => {
      napiMarshallingInOnly(items)
    }
  },
  {
    name: 'Q. NAPI output marshalling only (220KB Uint8Array)',
    fn: () => {
      napiMarshallingOutOnly(probeBuffer.byteLength)
    }
  }
]

console.log('Running 3 rounds for each phase…')
console.log('')
const results = await compareRobust(benches)

console.log('| Phase | p50 (spread) | per comment | Δ vs prev |')
console.log('|---|---:|---:|---:|')
let prev = null
for (const r of results) {
  const total = `${fmtDuration(r.p50)} (±${r.spread_pct.toFixed(1)}%)`
  const per = fmtDuration(r.p50 / allComments.length)
  let delta = '—'
  // Phases A→B→C are inclusive; D and X are independent reference points.
  if (prev && r.name.startsWith('B') && results[0].name.startsWith('A')) {
    delta = fmtDuration(r.p50 - results[0].p50)
  }
  if (r.name.startsWith('C')) {
    delta = `${fmtDuration(r.p50 - results[1].p50)} (B→C)`
  }
  if (r.name.startsWith('E')) {
    delta = `${fmtDuration(r.p50 - results[3].p50)} (D→E)`
  }
  console.log(`| ${r.name} | ${total} | ${per} | ${delta} |`)
  prev = r
}

console.log('')
console.log('### Phase decomposition')
console.log('')
const a = results[0].p50
const b = results[1].p50
const c = results[2].p50
const d = results[3].p50
const e = results[4].p50
const x = results[5].p50
const p = results[6].p50
const q = results[7].p50
console.log(`- Pure NAPI call (rust + marshalling): **${fmtDuration(a)}** (${(a / allComments.length).toFixed(0)} ns/comment)`)
console.log(`- new RemoteSourceFile (header + alloc): **${fmtDuration(b - a)}** (B - A)`)
console.log(`- .asts root materialization: **${fmtDuration(c - b)}** (C - B)`)
console.log(`- Wrapper overhead (items.map + result obj): **${fmtDuration(d - a)}** (D - A, includes new RemoteSourceFile)`)
console.log(`- Wrapper extra over RAW + SF + asts: **${fmtDuration(e - c)}** (E - C; ideally 0)`)
console.log(`- items.map remap pure cost: **${fmtDuration(x)}**`)
console.log('')
console.log('### NAPI marshalling decomposition (diagnostic)')
console.log('')
console.log(`- **Input marshalling only** (Vec<JsBatchItem>): ${fmtDuration(p)} (${(p / allComments.length).toFixed(0)} ns/comment)`)
console.log(`- **Output marshalling only** (220KB Uint8Array): ${fmtDuration(q)}`)
console.log(`- Sum (P + Q): ${fmtDuration(p + q)}`)
console.log(`- Total NAPI call (A): ${fmtDuration(a)}`)
console.log(`- Implied **Rust parse work**: ${fmtDuration(a - p - q)} (A - P - Q)`)
console.log(`- Compare to Rust criterion bench (parse_batch_to_bytes single batch): ~448 µs`)

function extractJsdocComments(filePath, source) {
  const result = parseSync(filePath, source)
  return result.comments
    .filter(c => c.type === 'Block' && c.value.startsWith('*'))
    .map(c => source.slice(c.start, c.end))
}
