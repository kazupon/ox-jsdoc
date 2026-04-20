/**
 * Phase 1.2d size reduction KPI — binary AST vs JSON serialization output.
 *
 * Measures the buffer size for both paths over the same fixture corpus and
 * reports the ratio. Required KPI threshold: binary / JSON ≤ 40%.
 *
 * Note: the typed AST `parse()` returns an in-memory plain object after
 * `JSON.parse(astJson)`; the wire-level JSON length is what NAPI sends, so
 * we re-serialize via `JSON.stringify` to recover an apples-to-apples size.
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

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturePath = path.join(repoRoot, 'fixtures/perf/source/typescript-checker.ts')

const sourceText = await readFile(fixturePath, 'utf8')
const allComments = extractJsdocComments(fixturePath, sourceText)

let totalJsonBytes = 0
let totalBinaryBytes = 0

for (const comment of allComments) {
  const typedResult = parseTyped(comment)
  const json = JSON.stringify(typedResult.ast)
  totalJsonBytes += Buffer.byteLength(json, 'utf8')

  const binaryResult = parseBinary(comment)
  totalBinaryBytes += binaryResult.sourceFile.view.byteLength
}

const ratio = (totalBinaryBytes / totalJsonBytes) * 100
const reductionPct = 100 - ratio

console.log(`Loaded ${allComments.length} JSDoc comments from typescript-checker.ts`)
console.log('')
console.log('| Path | Total bytes | Avg bytes/comment |')
console.log('|---|---:|---:|')
console.log(
  `| Typed (JSON.stringify) | ${totalJsonBytes.toLocaleString()} | ${(totalJsonBytes / allComments.length).toFixed(1)} |`
)
console.log(
  `| Binary AST | ${totalBinaryBytes.toLocaleString()} | ${(totalBinaryBytes / allComments.length).toFixed(1)} |`
)
console.log('')
console.log(`Binary / JSON ratio: ${ratio.toFixed(1)}%`)
console.log(`Reduction: ${reductionPct.toFixed(1)}% smaller`)
console.log(
  `Required KPI: binary / JSON <= 40% → ${ratio <= 40 ? '✅ MET' : '❌ NOT MET'}`
)

function extractJsdocComments(filePath, source) {
  const result = parseSync(filePath, source)
  return result.comments
    .filter(c => c.type === 'Block' && c.value.startsWith('*'))
    .map(c => source.slice(c.start, c.end))
}
