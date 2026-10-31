#!/usr/bin/env node
/**
 * JSDoc linter benchmark — diagnostics check, between the setup and the
 * hyperfine phases.
 *
 * hyperfine only times the commands (`--ignore-failure`, since the rules
 * report diagnostics), so a command that fails, or a linter that checks
 * fewer or more comments than the others, only looks faster or slower. This
 * runs each command once with JSON output, counts the diagnostics of the
 * JSDoc rules, and checks that the patterns that run the same
 * `eslint-plugin-jsdoc` rules (`SAME_RULES_PATTERNS`) report the same
 * diagnostics (file, line, rule and message). It exits with 1 when they
 * differ or a command fails, so that nothing is timed then.
 *
 * Writes `tasks/benchmark/results/jsdoc-linter-diagnostics-<fixture>.json`,
 * which `jsdoc-linter-report.mjs` adds to the report.
 *
 * Usage: node tasks/benchmark/scripts/jsdoc-linter-diagnostics.mjs [fixture ...]
 */

import { spawnSync } from 'node:child_process'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'

import {
  benchRoot,
  FIXTURES,
  PATTERNS,
  resultsRoot,
  RULE_SETS,
  SAME_RULES_PATTERNS,
  tmpRoot
} from './lib/jsdoc-linter.mjs'

const RULE_SET = 'combined'
const eslintBin = path.join(benchRoot, 'node_modules/eslint/bin/eslint.js')
const oxlintBin = path.join(benchRoot, 'node_modules/oxlint/bin/oxlint')
// The same flags as `jsdoc-linter-hyperfine.sh`.
const ESLINT_FLAGS = ['--no-config-lookup', '--no-warn-ignored']
const OXLINT_FLAGS = [
  '--disable-nested-config',
  '--disable-unicorn-plugin',
  '--disable-oxc-plugin',
  '--disable-typescript-plugin'
]
const MAX_EXAMPLES = 20

/**
 * @typedef {{ file: string, line: number, rule: string, message: string }} Diagnostic
 * @typedef {{ files: number, diagnostics: Diagnostic[], notices: string[] }} Run
 */

/** An error that this script found. Its stack is not useful. */
class DiagnosticsError extends Error {}

/**
 * @param {string} file
 * @returns {string}
 */
function toPosix(file) {
  return file.replaceAll('\\', '/')
}

/**
 * `jsdoc/empty-tags` (ESLint), `jsdoc(empty-tags)` (Oxlint's built-in
 * plugin) and `jsdoc-js(empty-tags)` (the fork as an Oxlint JS plugin) are
 * all `empty-tags`.
 * @param {string} id
 * @returns {string}
 */
function ruleName(id) {
  return id.replace(/^jsdoc\//u, '').replace(/^(?:jsdoc|jsdoc-js)\((.+)\)$/u, '$1')
}

/**
 * @param {string} fixtureDir
 * @param {string} configDir
 * @param {string[]} rules
 * @returns {Run}
 */
function runEslint(fixtureDir, configDir, rules) {
  const output = path.join(configDir, 'diagnostics.json')
  const result = spawnSync(
    process.execPath,
    [eslintBin, '--config', path.join(configDir, 'eslint.config.js'), ...ESLINT_FLAGS, '-f', 'json', '-o', output, '.'],
    { cwd: fixtureDir, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 }
  )
  // 1 means that the rules reported something; anything else is a failure.
  if (result.status !== 0 && result.status !== 1) {
    throw new DiagnosticsError(`eslint exited with ${result.status}\n${result.stderr}`.trimEnd())
  }
  const results = JSON.parse(readFileSync(output, 'utf8'))
  /** @type {Diagnostic[]} */
  const diagnostics = []
  /** @type {string[]} */
  const notices = []
  for (const fileResult of results) {
    const file = toPosix(path.relative(fixtureDir, fileResult.filePath))
    for (const message of fileResult.messages) {
      const rule = message.ruleId ? ruleName(message.ruleId) : null
      if (rule !== null && rules.includes(rule)) {
        diagnostics.push({ file, line: message.line, rule, message: message.message })
      } else {
        notices.push(`${file}:${message.line ?? 0} ${message.message.split('\n')[0]}`)
      }
    }
  }
  return { files: results.length, diagnostics, notices }
}

/**
 * @param {string} fixtureDir
 * @param {string} configDir
 * @param {string[]} rules
 * @returns {Run}
 */
function runOxlint(fixtureDir, configDir, rules) {
  const result = spawnSync(
    process.execPath,
    [oxlintBin, '--config', path.join(configDir, '.oxlintrc.json'), ...OXLINT_FLAGS, '-f', 'json', '.'],
    { cwd: fixtureDir, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 }
  )
  if (result.status !== 0 && result.status !== 1) {
    throw new DiagnosticsError(`oxlint exited with ${result.status}\n${result.stderr}`.trimEnd())
  }
  let output
  try {
    output = JSON.parse(result.stdout)
  } catch {
    throw new DiagnosticsError(`oxlint did not print JSON\n${result.stderr}${result.stdout}`.trimEnd())
  }
  writeFileSync(path.join(configDir, 'diagnostics.json'), result.stdout)
  /** @type {Diagnostic[]} */
  const diagnostics = []
  /** @type {string[]} */
  const notices = []
  for (const diagnostic of output.diagnostics) {
    const file = toPosix(diagnostic.filename)
    const line = diagnostic.labels?.[0]?.span?.line ?? 0
    // A diagnostic without a rule is a parse error, or an error thrown in a
    // JS plugin (Oxlint drops the other diagnostics of that file then; the
    // comparison below finds them missing).
    const rule = diagnostic.code ? ruleName(diagnostic.code) : null
    if (rule !== null && rules.includes(rule)) {
      diagnostics.push({ file, line, rule, message: diagnostic.message })
    } else {
      notices.push(`${file}:${line} ${diagnostic.message.split('\n')[0]}`)
    }
  }
  return { files: output.number_of_files, diagnostics, notices }
}

/**
 * @param {Diagnostic} diagnostic
 * @returns {string}
 */
function format(diagnostic) {
  return `${diagnostic.file}:${diagnostic.line} ${diagnostic.rule}: ${diagnostic.message}`
}

/**
 * @param {Diagnostic[]} expected
 * @param {Diagnostic[]} actual
 * @returns {{ missing: string[], extra: string[] }}
 */
function diff(expected, actual) {
  const counts = new Map()
  for (const diagnostic of expected) {
    const key = format(diagnostic)
    counts.set(key, (counts.get(key) ?? 0) + 1)
  }
  for (const diagnostic of actual) {
    const key = format(diagnostic)
    counts.set(key, (counts.get(key) ?? 0) - 1)
  }
  const missing = []
  const extra = []
  for (const [key, count] of counts) {
    for (let i = 0; i < count; i++) missing.push(key)
    for (let i = 0; i < -count; i++) extra.push(key)
  }
  return { missing: missing.sort(), extra: extra.sort() }
}

/**
 * @param {Diagnostic[]} diagnostics
 * @param {string[]} rules
 * @returns {Record<string, number>}
 */
function countByRule(diagnostics, rules) {
  const counts = Object.fromEntries(rules.map(rule => [rule, 0]))
  for (const { rule } of diagnostics) counts[rule]++
  return counts
}

const names = process.argv.slice(2)
for (const name of names) {
  if (!(name in FIXTURES)) {
    console.error(`Unknown fixture: ${name} (${Object.keys(FIXTURES).join(', ')})`)
    process.exit(2)
  }
}
const rules = RULE_SETS[RULE_SET]
mkdirSync(resultsRoot, { recursive: true })

let failed = false
for (const [fixtureName, fixture] of Object.entries(FIXTURES)) {
  if (names.length > 0 && !names.includes(fixtureName)) continue
  console.log(`\n=== fixture: ${fixtureName} | rule set: ${RULE_SET} ===`)

  /** @type {Record<string, Run>} */
  const runs = {}
  /** @type {string[]} */
  const problems = []
  for (const pattern of PATTERNS) {
    const configDir = path.join(tmpRoot, fixtureName, pattern, RULE_SET)
    const start = performance.now()
    try {
      runs[pattern] = pattern.startsWith('eslint')
        ? runEslint(fixture.dir, configDir, rules)
        : runOxlint(fixture.dir, configDir, rules)
    } catch (error) {
      const message = error instanceof DiagnosticsError ? error.message : String(error?.stack ?? error)
      problems.push(`${pattern} failed: ${message.split('\n').slice(0, 8).join('\n    ')}`)
      continue
    }
    const run = runs[pattern]
    const seconds = ((performance.now() - start) / 1000).toFixed(1)
    console.log(
      `${pattern}: ${run.diagnostics.length} diagnostics in ${run.files} files ` +
        `(${Object.entries(countByRule(run.diagnostics, rules)).map(([rule, n]) => `${rule} ${n}`).join(', ')}), ` +
        `${run.notices.length} other messages, ${seconds} s`
    )
  }

  /** @type {Record<string, { missing: number, extra: number }>} */
  const differences = {}
  const [reference, ...others] = SAME_RULES_PATTERNS
  if (runs[reference]) {
    for (const pattern of others) {
      if (!runs[pattern]) continue
      const { missing, extra } = diff(runs[reference].diagnostics, runs[pattern].diagnostics)
      differences[pattern] = { missing: missing.length, extra: extra.length }
      if (missing.length === 0 && extra.length === 0) continue
      problems.push(`${pattern} differs from ${reference}: ${missing.length} missing, ${extra.length} extra`)
      for (const key of missing.slice(0, MAX_EXAMPLES)) problems.push(`  only in ${reference}: ${key}`)
      for (const key of extra.slice(0, MAX_EXAMPLES)) problems.push(`  only in ${pattern}: ${key}`)
    }
  }

  const match = problems.length === 0
  writeFileSync(
    path.join(resultsRoot, `jsdoc-linter-diagnostics-${fixtureName}-${RULE_SET}.json`),
    JSON.stringify(
      {
        fixture: fixtureName,
        ruleSet: RULE_SET,
        rules,
        sameRulesPatterns: SAME_RULES_PATTERNS,
        match,
        differences,
        patterns: Object.fromEntries(
          Object.entries(runs).map(([pattern, run]) => [
            pattern,
            {
              files: run.files,
              diagnostics: run.diagnostics.length,
              byRule: countByRule(run.diagnostics, rules),
              otherMessages: run.notices.length,
              firstOtherMessages: run.notices.slice(0, 5)
            }
          ])
        )
      },
      null,
      2
    )
  )

  if (match) {
    console.log(`OK: ${SAME_RULES_PATTERNS.join(', ')} report the same diagnostics`)
  } else {
    failed = true
    console.log(`FAIL (${problems.length} lines)`)
    for (const problem of problems) console.log(`  ${problem}`)
  }
}

if (failed) {
  console.log('\nThe patterns do not do the same work, so their times are not comparable. Fix this before timing them.')
}
process.exitCode = failed ? 1 : 0
