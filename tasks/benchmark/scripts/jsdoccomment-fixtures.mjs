/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { bench, run } from 'mitata'
import { parseComment } from '@es-joy/jsdoccomment'
import { parseSync } from 'oxc-parser'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturesRoot = path.join(repoRoot, 'fixtures', 'perf')

const buckets = [
  'common',
  'description-heavy',
  'type-heavy',
  'special-tag',
  'malformed',
  'source',
  'toolchain',
]

const fixtures = await loadFixtures()

for (const fixture of fixtures) {
  bench(`${fixture.bucket}/${fixture.name}`, () => {
    for (const commentText of fixture.commentTexts) {
      parseComment(commentText)
    }
  })
}

const result = await run({
  format: 'quiet',
  print: () => {},
  colors: false,
  throw: true,
})

const rows = result.benchmarks.flatMap(benchmark =>
  benchmark.runs.map(r => ({
    fixture: r.name,
    avgNs: r.stats.avg,
    minNs: r.stats.min,
    p75Ns: r.stats.p75,
    p99Ns: r.stats.p99,
    maxNs: r.stats.max,
  })),
)

rows.sort((left, right) => left.fixture.localeCompare(right.fixture))

console.log('| Fixture | Avg | Min | p75 | p99 | Max |')
console.log('|---|---:|---:|---:|---:|---:|')
for (const row of rows) {
  console.log(
    `| \`${row.fixture}\` | ${formatNs(row.avgNs)} | ${formatNs(row.minNs)} | ${formatNs(row.p75Ns)} | ${formatNs(row.p99Ns)} | ${formatNs(row.maxNs)} |`,
  )
}

async function loadFixtures() {
  const allFixtures = []

  for (const bucket of buckets) {
    const bucketDir = path.join(fixturesRoot, bucket)
    let entries
    try {
      entries = await readdir(bucketDir, { withFileTypes: true })
    } catch {
      continue
    }
    for (const entry of entries) {
      if (!entry.isFile() || !isSupportedFixture(entry.name)) {
        continue
      }
      const filePath = path.join(bucketDir, entry.name)
      const sourceText = await readFile(filePath, 'utf8')
      const commentTexts = entry.name.endsWith('.jsdoc')
        ? [sourceText]
        : extractJsdocBlocksWithOxcParser(filePath, sourceText)
      if (commentTexts.length === 0) {
        continue
      }
      allFixtures.push({
        bucket,
        name: entry.name.replace(/\.(?:jsdoc|[cm]?[jt]sx?)$/, ''),
        sourceText,
        commentTexts,
      })
    }
  }

  return allFixtures
}

function isSupportedFixture(name) {
  return /\.(?:jsdoc|[cm]?[jt]sx?)$/.test(name)
}

function extractJsdocBlocksWithOxcParser(filePath, sourceText) {
  const result = parseSync(filePath, sourceText)
  return result.comments
    .filter(isJsdocComment)
    .map(comment => sourceText.slice(comment.start, comment.end))
}

function isJsdocComment(comment) {
  return comment.type === 'Block' && comment.value.startsWith('*')
}

function formatNs(value) {
  if (value >= 1000) {
    return `${(value / 1000).toFixed(3)} µs`
  }
  return `${value.toFixed(3)} ns`
}
