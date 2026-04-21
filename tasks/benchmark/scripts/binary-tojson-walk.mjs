/**
 * Repeated-access benchmark for the lazy decoder — exercises scenarios
 * where field caching (#1 in the JS-side optimization investigation) would
 * pay off:
 *
 * - `toJSON full file` — every getter exercised exactly once.
 * - `toJSON x2` — same toJSON twice; the second call should benefit if
 *   getters cache their results internally.
 * - `Walk x1` — single visitor pass over every tag's hot fields.
 * - `Walk x3` — three repeated passes; cache hits dominate after pass 1.
 * - `description x10 per node` — synthetic multi-access probe; with caching
 *   only the first read pays the helper cost.
 *
 * Variance is controlled by `lib/measure.mjs` (`compareRobust`).
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { parseSync } from 'oxc-parser'
import { parse as parseBinary } from 'ox-jsdoc-binary'

import { compareRobust, fmtDuration } from './lib/measure.mjs'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturePath = path.join(repoRoot, 'fixtures/perf/source/typescript-checker.ts')

const sourceText = await readFile(fixturePath, 'utf8')
const allComments = extractJsdocComments(fixturePath, sourceText)

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts`)
console.log('')

const scenarios = [
  {
    label: 'toJSON x1 full file',
    fn: () => {
      for (const c of allComments) {
        const ast = parseBinary(c).ast
        if (ast) ast.toJSON()
      }
    }
  },
  {
    label: 'toJSON x2 full file (caching test)',
    fn: () => {
      for (const c of allComments) {
        const ast = parseBinary(c).ast
        if (!ast) continue
        ast.toJSON()
        ast.toJSON()
      }
    }
  },
  {
    label: 'Walk x1 full file',
    fn: () => {
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
  },
  {
    label: 'Walk x3 full file (caching test)',
    fn: () => {
      for (const c of allComments) {
        const ast = parseBinary(c).ast
        if (!ast) continue
        for (let pass = 0; pass < 3; pass++) {
          for (const tag of ast.tags) {
            void tag.tag?.value
            void tag.rawType?.raw
            void tag.name?.raw
            void tag.description
          }
        }
      }
    }
  },
  {
    label: 'description x10 per node (synthetic)',
    fn: () => {
      for (const c of allComments) {
        const ast = parseBinary(c).ast
        if (!ast) continue
        for (let i = 0; i < 10; i++) void ast.description
        for (const tag of ast.tags) {
          for (let i = 0; i < 10; i++) void tag.description
        }
      }
    }
  }
]

const benches = scenarios.map(s => ({ name: s.label, fn: s.fn }))
const results = await compareRobust(benches)

console.log('| Scenario | p50 (spread) | per comment |')
console.log('|---|---:|---:|')
for (const r of results) {
  const total = `${fmtDuration(r.p50)} (±${r.spread_pct.toFixed(1)}%)`
  const per = fmtDuration(r.p50 / allComments.length)
  console.log(`| ${r.name} | ${total} | ${per} |`)
}

function extractJsdocComments(filePath, source) {
  const result = parseSync(filePath, source)
  return result.comments
    .filter(c => c.type === 'Block' && c.value.startsWith('*'))
    .map(c => source.slice(c.start, c.end))
}
