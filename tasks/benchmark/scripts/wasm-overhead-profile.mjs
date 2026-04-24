/**
 * Phase-by-phase profile of the WASM parse path overhead.
 *
 * Mirrors `napi-overhead-profile.mjs`: measures pure WASM call cost vs
 * `RemoteSourceFile` construction vs lazy `asts` materialization vs the
 * full wrapper. Scenarios use the per-comment `parse_jsdoc` path
 * (loop-style, 226 calls). The comparison highlights where the per-call
 * WASM crossing costs concentrate so we know whether a WASM `parseBatch`
 * (concat buffer) variant is worth introducing.
 *
 * Plus a parseBatch comparison: the current `parseBatch` wrapper feeds the
 * Rust side `Vec<String>` (one js→wasm String marshalling per item — the
 * NAPI analog dominated the call before we replaced it with concat-buffer
 * `parse_jsdoc_batch_raw`). Phases B/Bbatch let us quantify that gap.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { parseSync } from 'oxc-parser'
import {
  initWasm as initBinaryWasm,
  parse as parseBinaryWasm,
  parseBatch as parseBatchWasm
} from '@ox-jsdoc/wasm-binary'
import {
  parse_jsdoc as parseJsdocWasmRaw,
  parse_jsdoc_batch as parseJsdocBatchWasmRaw
} from '../../../wasm/ox-jsdoc-binary/pkg/ox_jsdoc_binary_wasm.js'
import { RemoteSourceFile } from '../../../packages/decoder/src/index.js'

import { compareRobust, fmtDuration } from './lib/measure.mjs'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturePath = path.join(repoRoot, 'fixtures/perf/source/typescript-checker.ts')

await initBinaryWasm(
  await readFile(path.join(repoRoot, 'wasm/ox-jsdoc-binary/pkg/ox_jsdoc_binary_wasm_bg.wasm'))
)

const sourceText = await readFile(fixturePath, 'utf8')
const allComments = extractJsdocComments(fixturePath, sourceText)
const items = allComments.map(c => ({ sourceText: c, baseOffset: 0 }))

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts`)
console.log('')

// Sanity probe: confirm raw parse path returns a usable handle before we
// run the bench loops.
const probeHandle = parseJsdocWasmRaw(allComments[0], null, null, null, null, null)
const probeLen = probeHandle.bufferLen()
probeHandle.free()
console.log(`Probe buffer size for first comment: ${probeLen} bytes`)
console.log('')

const benches = [
  {
    name: 'A. WASM raw call only (loop, free)',
    fn: () => {
      for (const c of allComments) {
        const h = parseJsdocWasmRaw(c, null, null, null, null, null)
        h.free()
      }
    }
  },
  {
    name: 'B. + bufferPtr/bufferLen view (loop)',
    fn: () => {
      for (const c of allComments) {
        const h = parseJsdocWasmRaw(c, null, null, null, null, null)
        void h.bufferPtr()
        void h.bufferLen()
        h.free()
      }
    }
  },
  {
    name: 'C. Full wrapper parse() (loop, free, no .ast)',
    fn: () => {
      for (const c of allComments) {
        const r = parseBinaryWasm(c)
        r.free()
      }
    }
  },
  {
    name: 'D. Full wrapper + .ast (loop, free)',
    fn: () => {
      for (const c of allComments) {
        const r = parseBinaryWasm(c)
        void r.ast
        r.free()
      }
    }
  },
  {
    name: 'E. parseBatch wrapper (single call, no .asts)',
    fn: () => {
      const r = parseBatchWasm(items)
      r.free()
    }
  },
  {
    name: 'F. parseBatch wrapper + .asts (single call)',
    fn: () => {
      const r = parseBatchWasm(items)
      void r.asts
      r.free()
    }
  },
  {
    name: 'G. Raw parseBatch only (Vec<String> marshalling)',
    fn: () => {
      const sourceTexts = items.map(it => it.sourceText)
      const baseOffsets = new Uint32Array(items.length)
      const h = parseJsdocBatchWasmRaw(sourceTexts, baseOffsets, null, null, null, null)
      h.free()
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
  if (prev) {
    delta = fmtDuration(r.p50 - prev.p50)
  }
  console.log(`| ${r.name} | ${total} | ${per} | ${delta} |`)
  prev = r
}

console.log('')
console.log('### Loop vs Batch (one-shot 226 comments)')
console.log('')
const a = results[0].p50
const c = results[2].p50
const d = results[3].p50
const e = results[4].p50
const f = results[5].p50
const g = results[6].p50
console.log(`- **Loop A** (raw call ×226): ${fmtDuration(a)}`)
console.log(`- **Loop D** (full wrapper ×226 + .ast): ${fmtDuration(d)}`)
console.log(`- **Batch E** (parseBatch single call, no .asts): ${fmtDuration(e)}`)
console.log(`- **Batch F** (parseBatch single call + .asts): ${fmtDuration(f)}`)
console.log(`- Δ Batch F - Loop D: ${fmtDuration(f - d)} ${f - d < 0 ? '(batch faster)' : '(loop faster)'}`)
console.log('')
console.log('### Batch G isolation')
console.log('')
console.log(`- **Raw parseBatch only** (Vec<String> in, no decoder): ${fmtDuration(g)}`)
console.log(`- Wrapper Batch F overhead over raw G: ${fmtDuration(f - g)}`)
console.log('')
console.log('### Per-call WASM crossing cost (Loop)')
console.log('')
console.log(`- A (raw call only): ${(a / allComments.length).toFixed(0)} ns/comment`)
console.log(`- D - A (wrapper + view + decoder + .ast): ${((d - a) / allComments.length).toFixed(0)} ns/comment overhead`)

function extractJsdocComments(filePath, source) {
  const result = parseSync(filePath, source)
  return result.comments
    .filter(c => c.type === 'Block' && c.value.startsWith('*'))
    .map(c => source.slice(c.start, c.end))
}
