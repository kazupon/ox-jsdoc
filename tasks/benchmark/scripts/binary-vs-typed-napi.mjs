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
const batch100 = allComments.slice(0, 100)

// Pick a comment whose length is closest to the median so the "single"
// scenario reflects a typical input rather than a degenerate /** */.
const median = pickMedianLength(allComments)
const single = median.text

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts`)
console.log(
  `Single comment: ${single.length} bytes (median; range ${median.min}-${median.max} across all comments)`
)
console.log(`Batch 100 cumulative length: ${batch100.reduce((a, c) => a + c.length, 0)} bytes`)
console.log('')

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

// Fair comparison: force `.ast` materialization on both sides. The typed
// wrapper exposes `ast` as a lazy getter that triggers `JSON.parse`, while
// the binary wrapper eagerly builds `RemoteSourceFile + asts[0]` inside
// `parse()`. Without the explicit access, the typed measurement skips the
// JSON.parse cost.

group('Single comment (typed vs binary, .ast accessed)', () => {
  bench('parseTyped (NAPI)', () => {
    void parseTyped(single).ast
  })
  bench('parseBinary (NAPI)', () => {
    void parseBinary(single).ast
  })
})

group('Batch 100 comments (typed vs binary, .ast accessed)', () => {
  bench('parseTyped (NAPI) x100', () => {
    for (const comment of batch100) void parseTyped(comment).ast
  })
  bench('parseBinary (NAPI) x100', () => {
    for (const comment of batch100) void parseBinary(comment).ast
  })
})

group(`Full file: ${allComments.length} comments (.ast accessed)`, () => {
  bench('parseTyped (NAPI) full', () => {
    for (const comment of allComments) void parseTyped(comment).ast
  })
  bench('parseBinary (NAPI) full', () => {
    for (const comment of allComments) void parseBinary(comment).ast
  })
})

// Sparse access — read a single root scalar (no child traversal). For
// typed this still pays the full JSON.parse; for binary it only touches
// the root node's Extended Data string. Closest match to the design's
// "Recommended: lazy sparse access" KPI (target: 1/10 of typed time).
//
// Note: `ast.tags.length` is NOT a sparse access for binary — the
// `tags` getter eagerly walks the NodeList and constructs every
// `RemoteJsdocTag` instance. Use a scalar field instead.
group('Sparse access — root.description only (full file)', () => {
  bench('parseTyped (NAPI) sparse', () => {
    for (const comment of allComments) void parseTyped(comment).ast?.description
  })
  bench('parseBinary (NAPI) sparse', () => {
    for (const comment of allComments) void parseBinary(comment).ast?.description
  })
})

// Full materialisation — walk every tag and read every accessor. Binary's
// worst case (forces RemoteJsdocTag construction per tag); typed already
// has the full object after JSON.parse.
group('Full materialisation — read every tag field (full file)', () => {
  bench('parseTyped (NAPI) walk', () => {
    for (const comment of allComments) {
      const ast = parseTyped(comment).ast
      if (!ast) continue
      const tags = ast.tags ?? []
      for (const tag of tags) {
        // touch every commonly-read scalar
        void tag.tag
        void tag.rawType
        void tag.name
        void tag.description
      }
    }
  })
  bench('parseBinary (NAPI) walk', () => {
    for (const comment of allComments) {
      const ast = parseBinary(comment).ast
      if (!ast) continue
      const tags = ast.tags
      for (const tag of tags) {
        void tag.tag?.value
        void tag.rawType?.raw
        void tag.name?.raw
        void tag.description
      }
    }
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
  ['parseTyped (NAPI) full', 'parseBinary (NAPI) full', `Full file (${allComments.length} comments)`],
  ['parseTyped (NAPI) sparse', 'parseBinary (NAPI) sparse', 'Sparse: root.description only'],
  ['parseTyped (NAPI) walk', 'parseBinary (NAPI) walk', 'Full walk: every tag field']
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
