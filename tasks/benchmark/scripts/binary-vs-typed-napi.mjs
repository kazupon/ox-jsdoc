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
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { bench, group, run } from 'mitata'
import { parseSync } from 'oxc-parser'
import { parse as parseTyped } from 'ox-jsdoc'
import { parse as parseBinary } from 'ox-jsdoc-binary'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturePath = path.join(repoRoot, 'fixtures/perf/source/typescript-checker.ts')

const sourceText = await readFile(fixturePath, 'utf8')
const allComments = extractJsdocComments(fixturePath, sourceText)
const single = allComments[0]
const batch100 = allComments.slice(0, 100)

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts`)
console.log(`Single comment length: ${single.length} bytes`)
console.log(`Batch 100 cumulative length: ${batch100.reduce((a, c) => a + c.length, 0)} bytes`)
console.log('')

group('Single comment (typed vs binary)', () => {
  bench('parseTyped (NAPI)', () => parseTyped(single))
  bench('parseBinary (NAPI)', () => parseBinary(single))
})

group('Batch 100 comments (typed vs binary)', () => {
  bench('parseTyped (NAPI) x100', () => {
    for (const comment of batch100) parseTyped(comment)
  })
  bench('parseBinary (NAPI) x100', () => {
    for (const comment of batch100) parseBinary(comment)
  })
})

group(`Full file: ${allComments.length} comments`, () => {
  bench('parseTyped (NAPI) full', () => {
    for (const comment of allComments) parseTyped(comment)
  })
  bench('parseBinary (NAPI) full', () => {
    for (const comment of allComments) parseBinary(comment)
  })
})

const result = await run({
  format: 'quiet',
  print: () => {},
  colors: false,
  throw: true
})

const rows = result.benchmarks.flatMap(b => b.runs.map(r => ({ name: r.name, avgNs: r.stats.avg })))

console.log('')
console.log('| Scenario | parseTyped | parseBinary | Speedup |')
console.log('|---|---:|---:|---:|')

const pairs = [
  ['parseTyped (NAPI)', 'parseBinary (NAPI)', 'Single comment'],
  ['parseTyped (NAPI) x100', 'parseBinary (NAPI) x100', 'Batch 100'],
  ['parseTyped (NAPI) full', 'parseBinary (NAPI) full', `Full file (${allComments.length} comments)`]
]
for (const [typedName, binaryName, label] of pairs) {
  const typed = rows.find(r => r.name === typedName)
  const binary = rows.find(r => r.name === binaryName)
  if (!typed || !binary) continue
  const speedup = typed.avgNs / binary.avgNs
  console.log(
    `| ${label} | ${formatNs(typed.avgNs)} | ${formatNs(binary.avgNs)} | **${speedup.toFixed(2)}x** |`
  )
}

function extractJsdocComments(filePath, source) {
  const result = parseSync(filePath, source)
  return result.comments
    .filter(c => c.type === 'Block' && c.value.startsWith('*'))
    .map(c => source.slice(c.start, c.end))
}

function formatNs(value) {
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(3)} ms`
  }
  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(3)} µs`
  }
  return `${value.toFixed(3)} ns`
}
