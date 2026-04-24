// Sanity: ensure both bindings see the same tag counts on the same fixture.
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { parseSync } from 'oxc-parser'
import { parse as parseTyped } from 'ox-jsdoc'
import { parse as parseBinary } from 'ox-jsdoc-binary'

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
const fixturePath = path.resolve(repoRoot, 'fixtures/perf/source/typescript-checker.ts')
const sourceText = await readFile(fixturePath, 'utf8')
const result = parseSync(fixturePath, sourceText)
const comments = result.comments
  .filter(c => c.type === 'Block' && c.value.startsWith('*'))
  .map(c => sourceText.slice(c.start, c.end))

let typedTags = 0
let binaryTags = 0
let typedDescLines = 0
let binaryDescLines = 0
let typedNullAst = 0
let binaryNullAst = 0
let typedJsonTotal = 0

for (const c of comments) {
  const t = parseTyped(c)
  const b = parseBinary(c)
  if (!t.ast) typedNullAst++
  if (!b.ast) binaryNullAst++
  if (t.ast) {
    typedTags += t.ast.tags?.length ?? 0
    typedDescLines += t.ast.descriptionLines?.length ?? 0
    typedJsonTotal += JSON.stringify(t.ast).length
  }
  if (b.ast) {
    binaryTags += b.ast.tags?.length ?? 0
    binaryDescLines += b.ast.descriptionLines?.length ?? 0
  }
}

console.log(`comments: ${comments.length}`)
console.log(`typed null asts: ${typedNullAst}, binary null asts: ${binaryNullAst}`)
console.log(`typed total tags: ${typedTags}, binary total tags: ${binaryTags}`)
console.log(`typed descLines: ${typedDescLines}, binary descLines: ${binaryDescLines}`)
console.log(`typed JSON total chars: ${typedJsonTotal}`)
