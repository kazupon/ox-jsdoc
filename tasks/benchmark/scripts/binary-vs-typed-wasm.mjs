/**
 * Phase 1.2d benchmark — typed AST vs binary AST through the WASM binding.
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
import { initWasm as initTypedWasm, parse as parseTypedWasm } from '@ox-jsdoc/wasm'
import { initWasm as initBinaryWasm, parse as parseBinaryWasm } from '@ox-jsdoc/wasm-binary'

import { compareRobust, fmtDuration } from './lib/measure.mjs'

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
console.log(
  `Single comment: ${single.length} bytes (median; range ${lengths[0]}-${lengths[lengths.length - 1]})`
)
console.log('')

const scenarios = [
  {
    label: 'Single comment',
    typed: () => {
      void parseTypedWasm(single).ast
    },
    binary: () => {
      const r = parseBinaryWasm(single)
      void r.ast
      r.free()
    }
  },
  {
    label: 'Batch 100',
    typed: () => {
      for (const c of batch100) void parseTypedWasm(c).ast
    },
    binary: () => {
      for (const c of batch100) {
        const r = parseBinaryWasm(c)
        void r.ast
        r.free()
      }
    }
  },
  {
    label: `Full file (${allComments.length} comments)`,
    typed: () => {
      for (const c of allComments) void parseTypedWasm(c).ast
    },
    binary: () => {
      for (const c of allComments) {
        const r = parseBinaryWasm(c)
        void r.ast
        r.free()
      }
    }
  },
  {
    label: 'Sparse: root.description only',
    typed: () => {
      for (const c of allComments) void parseTypedWasm(c).ast?.description
    },
    binary: () => {
      for (const c of allComments) {
        const r = parseBinaryWasm(c)
        void r.ast?.description
        r.free()
      }
    }
  },
  {
    label: 'Full walk: every tag field',
    typed: () => {
      for (const c of allComments) {
        const ast = parseTypedWasm(c).ast
        if (!ast) continue
        const tags = ast.tags ?? []
        for (const tag of tags) {
          void tag.tag
          void tag.rawType
          void tag.name
          void tag.description
        }
      }
    },
    binary: () => {
      for (const c of allComments) {
        const r = parseBinaryWasm(c)
        const ast = r.ast
        if (ast) {
          for (const tag of ast.tags) {
            void tag.tag?.value
            void tag.rawType?.raw
            void tag.name?.raw
            void tag.description
          }
        }
        r.free()
      }
    }
  }
]

const benches = []
for (const s of scenarios) {
  benches.push({ name: `typed | ${s.label}`, fn: s.typed })
  benches.push({ name: `binary | ${s.label}`, fn: s.binary })
}

const results = await compareRobust(benches)
const byName = new Map(results.map(r => [r.name, r]))

console.log('| Scenario | parseTyped (WASM, spread) | parseBinary (WASM, spread) | Speedup |')
console.log('|---|---:|---:|---:|')
for (const s of scenarios) {
  const t = byName.get(`typed | ${s.label}`)
  const b = byName.get(`binary | ${s.label}`)
  const speedup = t.p50 / b.p50
  console.log(
    `| ${s.label} | ${fmtDuration(t.p50)} (±${t.spread_pct.toFixed(1)}%) | ${fmtDuration(b.p50)} (±${b.spread_pct.toFixed(1)}%) | **${speedup.toFixed(2)}x** |`
  )
}

function extractJsdocComments(filePath, source) {
  const result = parseSync(filePath, source)
  return result.comments
    .filter(c => c.type === 'Block' && c.value.startsWith('*'))
    .map(c => source.slice(c.start, c.end))
}
