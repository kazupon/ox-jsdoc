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
const batch100 = allComments.slice(0, 100)

// Same median-length pick as the NAPI bench so the two reports are
// directly comparable.
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

// Same fairness fix as the NAPI bench: force `.ast` access on both sides
// so JSON.parse (typed) is included and we are not just timing the
// pre-decode stages.

group('Single comment (typed vs binary, WASM, .ast accessed)', () => {
  bench('parseTypedWasm', () => {
    void parseTypedWasm(single).ast
  })
  bench('parseBinaryWasm', () => {
    const r = parseBinaryWasm(single)
    void r.ast
    r.free()
  })
})

group('Batch 100 (typed vs binary, WASM, .ast accessed)', () => {
  bench('parseTypedWasm x100', () => {
    for (const c of batch100) void parseTypedWasm(c).ast
  })
  bench('parseBinaryWasm x100', () => {
    for (const c of batch100) {
      const r = parseBinaryWasm(c)
      void r.ast
      r.free()
    }
  })
})

group(`Full file (${allComments.length} comments, .ast accessed)`, () => {
  bench('parseTypedWasm full', () => {
    for (const c of allComments) void parseTypedWasm(c).ast
  })
  bench('parseBinaryWasm full', () => {
    for (const c of allComments) {
      const r = parseBinaryWasm(c)
      void r.ast
      r.free()
    }
  })
})

// True sparse — root scalar only, no child node materialisation. See the
// NAPI bench for the rationale.
group('Sparse access — root.description only (full file, WASM)', () => {
  bench('parseTypedWasm sparse', () => {
    for (const c of allComments) void parseTypedWasm(c).ast?.description
  })
  bench('parseBinaryWasm sparse', () => {
    for (const c of allComments) {
      const r = parseBinaryWasm(c)
      void r.ast?.description
      r.free()
    }
  })
})

group('Full walk — every tag field (full file, WASM)', () => {
  bench('parseTypedWasm walk', () => {
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
  })
  bench('parseBinaryWasm walk', () => {
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
  ['parseTypedWasm full', 'parseBinaryWasm full', `Full file (${allComments.length} comments)`],
  ['parseTypedWasm sparse', 'parseBinaryWasm sparse', 'Sparse: root.description only'],
  ['parseTypedWasm walk', 'parseBinaryWasm walk', 'Full walk: every tag field']
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
