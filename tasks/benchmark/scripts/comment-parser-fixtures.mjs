import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { bench, run } from 'mitata'
import { parse } from 'comment-parser'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const fixturesRoot = path.join(repoRoot, 'fixtures', 'perf')

const buckets = [
  'common',
  'description-heavy',
  'type-heavy',
  'special-tag',
  'malformed',
  'toolchain'
]

const fixtures = await loadFixtures()

for (const fixture of fixtures) {
  bench(`${fixture.bucket}/${fixture.name}`, () => {
    parse(fixture.sourceText)
  })
}

const result = await run({
  format: 'quiet',
  print: () => {},
  colors: false,
  throw: true
})

const rows = result.benchmarks.flatMap((benchmark) =>
  benchmark.runs.map((run) => ({
    fixture: run.name,
    avgNs: run.stats.avg,
    minNs: run.stats.min,
    p75Ns: run.stats.p75,
    p99Ns: run.stats.p99,
    maxNs: run.stats.max
  }))
)

rows.sort((left, right) => left.fixture.localeCompare(right.fixture))

console.log('| Fixture | Avg | Min | p75 | p99 | Max |')
console.log('|---|---:|---:|---:|---:|---:|')
for (const row of rows) {
  console.log(
    `| \`${row.fixture}\` | ${formatNs(row.avgNs)} | ${formatNs(row.minNs)} | ${formatNs(row.p75Ns)} | ${formatNs(row.p99Ns)} | ${formatNs(row.maxNs)} |`
  )
}

async function loadFixtures() {
  const allFixtures = []

  for (const bucket of buckets) {
    const bucketDir = path.join(fixturesRoot, bucket)
    const entries = await readdir(bucketDir, { withFileTypes: true })
    for (const entry of entries) {
      if (!entry.isFile() || !entry.name.endsWith('.jsdoc')) {
        continue
      }
      const filePath = path.join(bucketDir, entry.name)
      const sourceText = await readFile(filePath, 'utf8')
      allFixtures.push({
        bucket,
        name: entry.name.replace(/\.jsdoc$/, ''),
        sourceText
      })
    }
  }

  return allFixtures
}

function formatNs(value) {
  if (value >= 1000) {
    return `${(value / 1000).toFixed(3)} µs`
  }
  return `${value.toFixed(3)} ns`
}
