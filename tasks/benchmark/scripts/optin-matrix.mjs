/**
 * Phase 5 opt-in matrix — measure parseBatch median + completed buffer
 * size for all 4 (compatMode × preserveWhitespace) combinations + the
 * compat+emptyStringForNull variant. Mirrors §5 of the
 * 2026-04-26-phase-5-opt-in-preserve-whitespace.md report.
 */

import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { parseBatch } from 'ox-jsdoc-binary'

import { fmtDuration, measureRobust } from './lib/measure.mjs'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturePath = path.join(repoRoot, 'fixtures/perf/source/typescript-checker.ts')
const sourceText = await readFile(fixturePath, 'utf8')

const COMMENT_RE = /\/\*\*[\s\S]*?\*\//g
const allComments = sourceText.match(COMMENT_RE) ?? []
const items = allComments.map(text => ({ sourceText: text, baseOffset: 0 }))

const cases = [
  { label: '{} (basic, default)', options: {} },
  { label: '{ preserveWhitespace: true }', options: { preserveWhitespace: true } },
  { label: '{ compatMode: true }', options: { compatMode: true } },
  {
    label: '{ compatMode: true, preserveWhitespace: true }',
    options: { compatMode: true, preserveWhitespace: true }
  },
  {
    label: '{ compatMode: true, emptyStringForNull: true }',
    options: { compatMode: true, emptyStringForNull: true }
  }
]

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts\n`)
console.log('| Mode | Median time | Buffer size | Δ buffer vs basic |')
console.log('|---|---:|---:|---:|')

const baselineBuffer = parseBatch(items, cases[0].options).sourceFile.view.buffer.byteLength

for (const { label, options } of cases) {
  const stats = await measureRobust(() => {
    parseBatch(items, options)
  })
  const result = parseBatch(items, options)
  const bufLen = result.sourceFile.view.buffer.byteLength
  const delta = bufLen - baselineBuffer
  const deltaStr = delta === 0 ? '0 B' : `${delta > 0 ? '+' : ''}${delta.toLocaleString()} B`
  console.log(
    `| \`${label}\` | ${fmtDuration(stats.p50)} | ${bufLen.toLocaleString()} B | ${deltaStr} |`
  )
}
