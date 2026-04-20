/**
 * Phase 1.2d benchmark — typed AST vs binary AST through the NAPI binding.
 *
 * Measures the Required KPIs from
 * `design/007-binary-ast/benchmark.md#phase-13-cutover-decision-primary-kpis`:
 *
 * - parse time (single comment)
 * - parse time (batch of 100)
 * - end-to-end (parse(text) total)
 *
 * `parseTyped` = `ox-jsdoc` (typed AST + JSON.parse round-trip)
 * `parseBinary` = `ox-jsdoc-binary` (binary AST + lazy decoder)
 *
 * Uses `lib/measure.mjs` (median-of-rounds with trimmed mean) instead of
 * mitata's `bench`/`group`/`run` so single-round noise (kernel preemption,
 * GC pauses, thermal cycle) does not dominate the answer. Each scenario
 * also reports the spread between the best and worst round so the reader
 * can judge result reliability at a glance.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { parseSync } from 'oxc-parser'
import { parse as parseTyped } from 'ox-jsdoc'
import { parse as parseBinary } from 'ox-jsdoc-binary'

import { compareRobust, fmtDuration } from './lib/measure.mjs'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturePath = path.join(repoRoot, 'fixtures/perf/source/typescript-checker.ts')

const sourceText = await readFile(fixturePath, 'utf8')
const allComments = extractJsdocComments(fixturePath, sourceText)
const batch100 = allComments.slice(0, 100)

const median = pickMedianLength(allComments)
const single = median.text

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts`)
console.log(
  `Single comment: ${single.length} bytes (median; range ${median.min}-${median.max} across all comments)`
)
console.log(`Batch 100 cumulative length: ${batch100.reduce((a, c) => a + c.length, 0)} bytes`)
console.log('')

const scenarios = [
  {
    label: 'Single comment',
    typed: () => {
      void parseTyped(single).ast
    },
    binary: () => {
      void parseBinary(single).ast
    }
  },
  {
    label: 'Batch 100',
    typed: () => {
      for (const c of batch100) void parseTyped(c).ast
    },
    binary: () => {
      for (const c of batch100) void parseBinary(c).ast
    }
  },
  {
    label: `Full file (${allComments.length} comments)`,
    typed: () => {
      for (const c of allComments) void parseTyped(c).ast
    },
    binary: () => {
      for (const c of allComments) void parseBinary(c).ast
    }
  },
  {
    label: 'Sparse: root.description only',
    typed: () => {
      for (const c of allComments) void parseTyped(c).ast?.description
    },
    binary: () => {
      for (const c of allComments) void parseBinary(c).ast?.description
    }
  },
  {
    label: 'Full walk: every tag field',
    typed: () => {
      for (const c of allComments) {
        const ast = parseTyped(c).ast
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
        const ast = parseBinary(c).ast
        if (!ast) continue
        for (const tag of ast.tags) {
          void tag.tag?.value
          void tag.rawType?.raw
          void tag.name?.raw
          void tag.description
        }
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

console.log('| Scenario | parseTyped (spread) | parseBinary (spread) | Speedup |')
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

function pickMedianLength(comments) {
  const lengths = comments.map(c => c.length).sort((a, b) => a - b)
  const target = lengths[Math.floor(lengths.length / 2)]
  let chosen = comments[0]
  let bestDelta = Math.abs(comments[0].length - target)
  for (const c of comments) {
    const d = Math.abs(c.length - target)
    if (d < bestDelta) {
      chosen = c
      bestDelta = d
    }
  }
  return { text: chosen, min: lengths[0], max: lengths[lengths.length - 1] }
}
