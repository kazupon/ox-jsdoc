#!/usr/bin/env node
/**
 * JSDoc linter benchmark — report aggregator.
 *
 * Reads `fixture-stats.json` and the 2 (fixtures) × 1 (rule set) = 2
 * hyperfine JSON exports, then writes a single combined Markdown report
 * at `tasks/benchmark/results/jsdoc-linter-hyperfine.md` (and the merged
 * JSON at `jsdoc-linter-hyperfine.json`).
 *
 * Run after `jsdoc-linter-hyperfine.sh` finishes.
 */

import { readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const benchRoot = path.resolve(__dirname, '..')
const tmpRoot = path.join(benchRoot, '.tmp/jsdoc-linter')
const resultsRoot = path.join(benchRoot, 'results')

const FIXTURES = ['js', 'ts']
const RULE_SETS = ['combined']

const stats = JSON.parse(readFileSync(path.join(tmpRoot, 'fixture-stats.json'), 'utf8'))

/** @type {Record<string, Record<string, object>>} */
const resultsByFixture = {}
for (const fx of FIXTURES) {
  resultsByFixture[fx] = {}
  for (const rs of RULE_SETS) {
    const jsonPath = path.join(resultsRoot, `jsdoc-linter-hyperfine-${fx}-${rs}.json`)
    resultsByFixture[fx][rs] = JSON.parse(readFileSync(jsonPath, 'utf8'))
  }
}

function formatDuration(seconds) {
  const ms = seconds * 1000
  if (ms >= 1000) return `${(ms / 1000).toFixed(3)} s`
  return `${ms.toFixed(1)} ms`
}

function percentile(values, p) {
  if (values.length === 0) return 0
  const sorted = [...values].sort((a, b) => a - b)
  const index = (sorted.length - 1) * p
  const lower = Math.floor(index)
  const upper = Math.ceil(index)
  if (lower === upper) return sorted[lower]
  const weight = index - lower
  return sorted[lower] * (1 - weight) + sorted[upper] * weight
}

function buildMarkdown() {
  const today = new Date().toISOString().slice(0, 10)
  const lines = []
  lines.push(`# ${today} — JSDoc linter hyperfine benchmark`)
  lines.push('')
  lines.push(
    'End-to-end CLI measurement (shell driver + direct hyperfine, `oxc-project/bench-linter` style) based on `design/009-jsdoc-linter-benchmark/README.md`.'
  )
  lines.push('')
  lines.push(
    '**2 fixtures × 5 patterns × 1 rule set (combined) = 10 measurement points**, covering both JS and TS against real multi-file projects.'
  )
  lines.push(
    '`combined` rule set = `jsdoc/empty-tags` + `jsdoc/require-param-description` + `jsdoc/require-param-type` (representative of a practical lint set).'
  )
  lines.push('')

  // ---- Fixture stats ----
  lines.push('## Fixtures')
  lines.push('')
  lines.push('| Fixture | Path | Files | Lines | JSDoc blocks | `@param` (with type / desc) |')
  lines.push('|---|---|---:|---:|---:|---|')
  for (const fx of FIXTURES) {
    const s = stats[fx]
    lines.push(
      `| \`${fx}\` | \`${s.fixtureDir}/\` | ${s.fileCount.toLocaleString()} | ${s.totalLines.toLocaleString()} | ${s.blockCount.toLocaleString()} | ${s.paramCount} (${s.paramWithType} / ${s.paramWithDescription}) |`
    )
  }
  lines.push('')
  lines.push(
    "- `js` fixture: lint the `eslint-plugin-jsdoc` source with the ESLint default parser (espree)"
  )
  lines.push(
    "- `ts` fixture: linting VS Code's TS source with ESLint requires `@typescript-eslint/parser` (Oxlint is TS-native)"
  )
  lines.push('')

  // ---- Patterns ----
  lines.push('## Patterns')
  lines.push('')
  lines.push('| # | Name | Linter | JSDoc parser / strategy |')
  lines.push('|---|---|---|---|')
  lines.push(
    '| 1 | `eslint-jsdoc-upstream` | ESLint | upstream `eslint-plugin-jsdoc` (`@es-joy/jsdoccomment`) |'
  )
  lines.push('| 2 | `oxlint-jsdoc-native` | Oxlint | built-in JSDoc plugin (Rust) |')
  lines.push(
    "| 3 | `eslint-ox-jsdoc-single` | ESLint | `@ox-jsdoc/eslint-plugin-jsdoc` + `oxParseStrategy: 'single'` |"
  )
  lines.push(
    "| 4 | `eslint-ox-jsdoc-batch` | ESLint | `@ox-jsdoc/eslint-plugin-jsdoc` + `oxParseStrategy: 'batch'` |"
  )
  lines.push(
    "| 5 | `oxlint-ox-jsdoc-batch` | Oxlint (JS plugin bridge, alias `jsdoc-js`) | `@ox-jsdoc/eslint-plugin-jsdoc` + `oxParseStrategy: 'batch'` |"
  )
  lines.push('')

  // ---- Per-fixture results (combined rule set only) ----
  for (const fx of FIXTURES) {
    const s = stats[fx]
    lines.push(`## Fixture: \`${fx}\` — ${s.fixtureDir}`)
    lines.push('')
    const hf = resultsByFixture[fx].combined
    const baselineMean = hf.results.find(r => r.command === 'eslint-jsdoc-upstream')?.mean
    lines.push('| # | Name | Mean | Median | p95 | Stddev | vs baseline |')
    lines.push('|---|---|---:|---:|---:|---:|---:|')
    for (let i = 0; i < hf.results.length; i++) {
      const r = hf.results[i]
      const ratio = baselineMean ? r.mean / baselineMean : null
      const ratioCol = ratio == null ? '—' : `${ratio.toFixed(2)}x`
      const p95 = percentile(r.times ?? [], 0.95)
      lines.push(
        `| ${i + 1} | \`${r.command}\` | ${formatDuration(r.mean)} | ${formatDuration(
          r.median ?? r.mean
        )} | ${formatDuration(p95)} | ${formatDuration(
          r.stddev ?? 0
        )} | ${ratioCol} |`
      )
    }
    lines.push('')
  }

  // ---- Cross-fixture combined comparison ----
  lines.push('## Cross-fixture summary')
  lines.push('')
  lines.push('| # | Pattern | js mean | ts mean | ts/js ratio |')
  lines.push('|---|---|---:|---:|---:|')
  const jsCombined = resultsByFixture.js.combined.results
  const tsCombined = resultsByFixture.ts.combined.results
  for (let i = 0; i < jsCombined.length; i++) {
    const j = jsCombined[i]
    const t = tsCombined[i]
    const ratio = j.mean ? t.mean / j.mean : 0
    lines.push(
      `| ${i + 1} | \`${j.command}\` | ${formatDuration(j.mean)} | ${formatDuration(t.mean)} | ${ratio.toFixed(2)}x |`
    )
  }
  lines.push('')

  return lines.join('\n') + '\n'
}

const mdPath = path.join(resultsRoot, 'jsdoc-linter-hyperfine.md')
const jsonPath = path.join(resultsRoot, 'jsdoc-linter-hyperfine.json')
writeFileSync(mdPath, buildMarkdown())
writeFileSync(jsonPath, JSON.stringify({ stats, results: resultsByFixture }, null, 2))
console.log(`→ ${path.relative(repoRoot, mdPath)}`)
console.log(`→ ${path.relative(repoRoot, jsonPath)}`)
