#!/usr/bin/env node
/**
 * Compare the diagnostics of `@ox-jsdoc/eslint-plugin-jsdoc` between ESLint
 * and Oxlint.
 *
 * Lints `fixtures/lint-parity/` with the same rules in both linters, once for
 * each `oxParseStrategy` (`single` and `batch`). Exits with 1 when the
 * diagnostics (file, line, rule, message) differ.
 *
 * Oxlint runs the plugin as a JS plugin. It shares one `SourceCode` object
 * between files and reuses objects such as comments, so a rule that keeps
 * state outside `create()` can work in ESLint but miss reports in Oxlint.
 *
 * Usage: vp run @ox-jsdoc/benchmark#jsdoc-linter:parity
 */

import { spawnSync } from 'node:child_process'
import { mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

import jsdoc from '@ox-jsdoc/eslint-plugin-jsdoc'
import { ESLint } from 'eslint'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const benchRoot = path.resolve(__dirname, '..')
const repoRoot = path.resolve(benchRoot, '../..')
const fixtureDir = path.join(repoRoot, 'fixtures/lint-parity')
const tmpRoot = path.join(benchRoot, '.tmp/jsdoc-linter-parity')
const oxlintBin = path.join(benchRoot, 'node_modules/oxlint/bin/oxlint')

const STRATEGIES = ['single', 'batch']

/**
 * Rules that `flat/recommended` leaves off but that work without options.
 * Together with the recommended rules, the set covers rules built on
 * `iterateAllJsdocs` and rules that are not.
 */
const EXTRA_RULES = [
  'check-indentation',
  'check-line-alignment',
  'check-template-names',
  'informative-docs',
  'lines-before-block',
  'no-bad-blocks',
  'no-blank-block-descriptions',
  'no-blank-blocks',
  'require-asterisk-prefix',
  'require-description',
  'require-description-complete-sentence',
  'require-hyphen-before-param-description',
  'require-template',
  'require-throws',
  'sort-tags'
]

/**
 * A plugin that reports the `oxParseStrategy` setting once per file. It
 * shows that each linter passes the setting through to the plugin, so the
 * `batch` run really uses the batch parser.
 */
const PROBE_PLUGIN = `export default {
  meta: { name: 'parity-probe' },
  rules: {
    'parse-strategy': {
      meta: { type: 'problem', schema: [] },
      create(context) {
        return {
          Program(node) {
            context.report({ node, message: 'oxParseStrategy: ' + context.settings.jsdoc?.oxParseStrategy })
          }
        }
      }
    }
  }
}
`
const PROBE_RULE = 'parity-probe/parse-strategy'

const OXLINT_CATEGORIES_OFF = {
  correctness: 'off',
  nursery: 'off',
  pedantic: 'off',
  perf: 'off',
  restriction: 'off',
  style: 'off',
  suspicious: 'off'
}

/**
 * @typedef {{ file: string, line: number, rule: string, message: string }} Diagnostic
 */

/** An error that this script found. Its stack is not useful. */
class ParityError extends Error {}

/**
 * @returns {string[]}
 */
function getRules() {
  const recommended = Object.entries(jsdoc.configs['flat/recommended'].rules)
    .filter(([, value]) => (Array.isArray(value) ? value[0] : value) !== 'off')
    .map(([name]) => name.replace(/^jsdoc\//, ''))
  const rules = [...new Set([...recommended, ...EXTRA_RULES])].sort()
  for (const rule of rules) {
    if (!(rule in jsdoc.rules)) {
      throw new Error(`Unknown rule: ${rule}`)
    }
  }
  return rules
}

/**
 * @param {string} file
 * @returns {string}
 */
function toPosix(file) {
  return file.replaceAll('\\', '/')
}

/**
 * @param {string} dir
 * @returns {string[]}
 */
function listFixtureFiles(dir) {
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter(entry => entry.isFile() && entry.name.endsWith('.js'))
    .map(entry => toPosix(path.relative(fixtureDir, path.join(entry.parentPath, entry.name))))
    .sort()
}

/**
 * @param {string} strategy
 * @param {string[]} rules
 * @param {string} probePath
 * @returns {Promise<{ diagnostics: Diagnostic[], files: string[] }>}
 */
async function runEslint(strategy, rules, probePath) {
  const { default: probe } = await import(pathToFileURL(probePath).href)
  const eslint = new ESLint({
    cwd: fixtureDir,
    overrideConfigFile: true,
    overrideConfig: [
      {
        files: ['**/*.js'],
        plugins: { jsdoc, 'parity-probe': probe },
        settings: { jsdoc: { oxParseStrategy: strategy } },
        linterOptions: { reportUnusedDisableDirectives: 'off' },
        rules: {
          [PROBE_RULE]: 'warn',
          ...Object.fromEntries(rules.map(rule => [`jsdoc/${rule}`, 'warn']))
        }
      }
    ]
  })
  const results = await eslint.lintFiles(['.'])
  /** @type {Diagnostic[]} */
  const diagnostics = []
  for (const result of results) {
    const file = toPosix(path.relative(fixtureDir, result.filePath))
    for (const message of result.messages) {
      if (!message.ruleId) {
        throw new ParityError(`ESLint could not lint ${file}: ${message.message}`)
      }
      diagnostics.push({
        file,
        line: message.line,
        rule: message.ruleId.replace(/^jsdoc\//, ''),
        message: message.message
      })
    }
  }
  return { diagnostics, files: results.map(result => toPosix(path.relative(fixtureDir, result.filePath))).sort() }
}

/**
 * @param {string} strategy
 * @param {string[]} rules
 * @param {string} dir
 * @returns {{ diagnostics: Diagnostic[], fileCount: number }}
 */
function runOxlint(strategy, rules, dir) {
  const configPath = path.join(dir, '.oxlintrc.json')
  const config = {
    jsPlugins: [
      // `jsdoc` is the name of Oxlint's built-in plugin, so use an alias.
      { name: 'jsdoc-js', specifier: '@ox-jsdoc/eslint-plugin-jsdoc' },
      { name: 'parity-probe', specifier: './probe.mjs' }
    ],
    categories: OXLINT_CATEGORIES_OFF,
    settings: { jsdoc: { oxParseStrategy: strategy } },
    rules: {
      [PROBE_RULE]: 'warn',
      ...Object.fromEntries(rules.map(rule => [`jsdoc-js/${rule}`, 'warn']))
    }
  }
  writeFileSync(configPath, JSON.stringify(config, null, 2))

  const result = spawnSync(
    process.execPath,
    [oxlintBin, '-c', configPath, '--disable-nested-config', '-f', 'json', '.'],
    { cwd: fixtureDir, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }
  )
  let output
  try {
    output = JSON.parse(result.stdout)
  } catch {
    throw new ParityError(`oxlint exited with ${result.status}\n${result.stderr}${result.stdout}`.trimEnd())
  }
  // Oxlint reports an error thrown in a JS plugin as a diagnostic without a
  // rule, and drops the other diagnostics of that file.
  const errors = output.diagnostics.filter(diagnostic => !diagnostic.code)
  if (errors.length > 0) {
    const lines = errors.map(error => error.message.split('\n').slice(0, 3).join(' | '))
    throw new ParityError(`${errors.length} errors\n${lines.join('\n')}`)
  }
  if (result.status !== 0) {
    throw new ParityError(`oxlint exited with ${result.status}\n${result.stderr}`.trimEnd())
  }
  /** @type {Diagnostic[]} */
  const diagnostics = output.diagnostics.map(diagnostic => {
    const match = /^(?<plugin>[^(]+)\((?<rule>.+)\)$/.exec(diagnostic.code ?? '')
    if (!match) {
      throw new ParityError(`Unexpected Oxlint diagnostic: ${JSON.stringify(diagnostic)}`)
    }
    const { plugin, rule } = /** @type {{ plugin: string, rule: string }} */ (match.groups)
    return {
      file: toPosix(diagnostic.filename),
      line: diagnostic.labels?.[0]?.span.line ?? 0,
      rule: plugin === 'jsdoc-js' ? rule : `${plugin}/${rule}`,
      message: diagnostic.message
    }
  })
  return { diagnostics, fileCount: output.number_of_files }
}

/**
 * @param {Diagnostic} diagnostic
 * @returns {string}
 */
function format(diagnostic) {
  return `${diagnostic.file}:${diagnostic.line} ${diagnostic.rule}: ${diagnostic.message}`
}

/**
 * Shortens long messages (such as the tag order of `sort-tags`) for the log.
 * @param {string} text
 * @returns {string}
 */
function truncate(text) {
  return text.length > 160 ? `${text.slice(0, 157)}...` : text
}

/**
 * @template T
 * @param {string} name
 * @param {() => Promise<T> | T} run
 * @returns {Promise<{ value: T } | { error: string }>}
 */
async function attempt(name, run) {
  try {
    return { value: await run() }
  } catch (error) {
    const message =
      error instanceof ParityError || !(error instanceof Error)
        ? String(error instanceof Error ? error.message : error)
        : (error.stack ?? error.message)
    return { error: `${name} failed: ${message.split('\n').slice(0, 8).join('\n    ')}` }
  }
}

/**
 * @param {Diagnostic[]} diagnostics
 * @returns {Map<string, number>}
 */
function countByKey(diagnostics) {
  const counts = new Map()
  for (const diagnostic of diagnostics) {
    const key = format(diagnostic)
    counts.set(key, (counts.get(key) ?? 0) + 1)
  }
  return counts
}

/**
 * @param {Diagnostic[]} left
 * @param {Diagnostic[]} right
 * @returns {{ onlyLeft: string[], onlyRight: string[] }}
 */
function diff(left, right) {
  const leftCounts = countByKey(left)
  const rightCounts = countByKey(right)
  const onlyLeft = []
  const onlyRight = []
  for (const key of new Set([...leftCounts.keys(), ...rightCounts.keys()])) {
    const delta = (leftCounts.get(key) ?? 0) - (rightCounts.get(key) ?? 0)
    for (let i = 0; i < delta; i++) onlyLeft.push(key)
    for (let i = 0; i < -delta; i++) onlyRight.push(key)
  }
  return { onlyLeft: onlyLeft.sort(), onlyRight: onlyRight.sort() }
}

/**
 * @param {Diagnostic[]} diagnostics
 * @returns {string}
 */
function summarizeRules(diagnostics) {
  const counts = new Map()
  for (const { rule } of diagnostics) {
    if (rule === PROBE_RULE) continue
    counts.set(rule, (counts.get(rule) ?? 0) + 1)
  }
  return [...counts.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([rule, count]) => `${rule} ${count}`)
    .join(', ')
}

/**
 * @param {string} strategy
 * @param {Diagnostic[]} diagnostics
 * @param {string[]} files
 * @returns {string[]}
 */
function checkProbe(strategy, diagnostics, files) {
  const expected = `oxParseStrategy: ${strategy}`
  const probed = new Map(
    diagnostics.filter(d => d.rule === PROBE_RULE).map(d => [d.file, d.message])
  )
  return files
    .filter(file => probed.get(file) !== expected)
    .map(file => `${file}: expected "${expected}", got "${probed.get(file) ?? '(no report)'}"`)
}

const rules = getRules()
const files = listFixtureFiles(fixtureDir)
console.log(`fixture: ${toPosix(path.relative(repoRoot, fixtureDir))} (${files.length} files)`)
console.log(`rules (${rules.length}): ${rules.join(', ')}`)

rmSync(tmpRoot, { recursive: true, force: true })

let failed = false
for (const strategy of STRATEGIES) {
  const dir = path.join(tmpRoot, strategy)
  mkdirSync(dir, { recursive: true })
  const probePath = path.join(dir, 'probe.mjs')
  writeFileSync(probePath, PROBE_PLUGIN)

  const eslintRun = await attempt('ESLint', () => runEslint(strategy, rules, probePath))
  const oxlintRun = await attempt('Oxlint', () => runOxlint(strategy, rules, dir))

  console.log(`\n== oxParseStrategy: ${strategy} ==`)
  /** @type {string[]} */
  const problems = []
  if ('error' in eslintRun) problems.push(eslintRun.error)
  if ('error' in oxlintRun) problems.push(oxlintRun.error)

  if ('value' in eslintRun && 'value' in oxlintRun) {
    const eslint = eslintRun.value
    const oxlint = oxlintRun.value
    console.log(`ESLint: ${eslint.diagnostics.length} diagnostics in ${eslint.files.length} files`)
    console.log(`Oxlint: ${oxlint.diagnostics.length} diagnostics in ${oxlint.fileCount} files`)
    console.log(`ESLint by rule: ${summarizeRules(eslint.diagnostics)}`)

    if (eslint.files.join() !== files.join()) {
      problems.push(`ESLint linted ${eslint.files.length} of ${files.length} fixture files`)
    }
    if (oxlint.fileCount !== files.length) {
      problems.push(`Oxlint linted ${oxlint.fileCount} of ${files.length} fixture files`)
    }
    problems.push(...checkProbe(strategy, eslint.diagnostics, files).map(p => `ESLint probe: ${p}`))
    problems.push(...checkProbe(strategy, oxlint.diagnostics, files).map(p => `Oxlint probe: ${p}`))

    const { onlyLeft, onlyRight } = diff(eslint.diagnostics, oxlint.diagnostics)
    for (const key of onlyLeft) problems.push(`only in ESLint: ${truncate(key)}`)
    for (const key of onlyRight) problems.push(`only in Oxlint: ${truncate(key)}`)
  }

  if (problems.length > 0) {
    failed = true
    console.log(`FAIL (${problems.length} problems)`)
    for (const problem of problems) console.log(`  ${problem}`)
  } else {
    console.log('OK: the diagnostics match')
  }
}

process.exitCode = failed ? 1 : 0
