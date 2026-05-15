#!/usr/bin/env node
/**
 * JSDoc linter benchmark — setup phase.
 *
 * Generates 2 (fixtures) × 5 (patterns) × 1 (rule set) = 10 lint configs
 * into `tasks/benchmark/.tmp/jsdoc-linter/<fixture>/<pattern>/<rule-set>/`
 * and writes `fixture-stats.json` next to them. Run before
 * `jsdoc-linter-hyperfine.sh`.
 *
 * Fixtures:
 *   js — refers/eslint-plugin-jsdoc/src/    (.js, espree default parser)
 *   ts — refers/vscode/src/                 (.ts, @typescript-eslint/parser)
 *
 * The setup is intentionally separated from hyperfine invocation: the
 * shell driver calls hyperfine directly to keep the per-command launch
 * path identical to upstream linter benchmarks (e.g.
 * `oxc-project/bench-linter`) — no Node.js / spawnSync wrapping.
 */

import { mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { parse as parseJsdoc } from 'ox-jsdoc'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(__dirname, '../../..')
const benchRoot = path.resolve(__dirname, '..')
const tmpRoot = path.join(benchRoot, '.tmp/jsdoc-linter')

/**
 * Two fixture variants — one JS, one TS. They share the rule sets and
 * pattern matrix but differ in extension glob and (for ESLint) parser.
 */
export const FIXTURES = {
  js: {
    dir: path.join(repoRoot, 'refers/eslint-plugin-jsdoc/src'),
    glob: '**/*.js',
    extensions: ['.js', '.cjs', '.mjs'],
    eslintParser: null
  },
  ts: {
    dir: path.join(repoRoot, 'refers/vscode/src'),
    glob: '**/*.ts',
    extensions: ['.ts'],
    eslintParser: '@typescript-eslint/parser'
  }
}

export const RULE_SETS = {
  combined: ['empty-tags', 'require-param-description', 'require-param-type']
}

export const PATTERNS = [
  'eslint-jsdoc-upstream',
  'oxlint-jsdoc-native',
  'eslint-ox-jsdoc-single',
  'eslint-ox-jsdoc-batch',
  'oxlint-ox-jsdoc-batch'
]

// ---------------------------------------------------------------------------
// Fixture stats
// ---------------------------------------------------------------------------

const COMMENT_RE = /\/\*\*[\s\S]*?\*\//g

function walkSourceFiles(dir, extensions) {
  /** @type {string[]} */
  const out = []
  for (const entry of readdirSync(dir)) {
    if (entry.startsWith('.') || entry === 'node_modules') continue
    const full = path.join(dir, entry)
    const stat = statSync(full)
    if (stat.isDirectory()) {
      out.push(...walkSourceFiles(full, extensions))
    } else if (extensions.some(ext => entry.endsWith(ext))) {
      out.push(full)
    }
  }
  return out
}

function computeFixtureStats(fixtureDir, extensions) {
  const files = walkSourceFiles(fixtureDir, extensions)

  let totalLines = 0
  let blockCount = 0
  let paramCount = 0
  let paramWithDescription = 0
  let paramWithType = 0
  let emptyTagCount = 0

  for (const file of files) {
    const sourceText = readFileSync(file, 'utf8')
    totalLines += sourceText.split('\n').length
    const comments = sourceText.match(COMMENT_RE) ?? []

    for (const comment of comments) {
      const { ast } = parseJsdoc(comment, {
        compatMode: true,
        emptyStringForNull: true,
        preserveWhitespace: true
      })
      if (!ast) continue
      blockCount++

      const block = ast.toJSON()
      for (const tag of block.tags ?? []) {
        const tagName = tag.tag ?? ''
        const hasType =
          (typeof tag.rawType === 'string' && tag.rawType !== '') ||
          (Array.isArray(tag.typeLines) && tag.typeLines.length > 0)
        const descLines = Array.isArray(tag.descriptionLines) ? tag.descriptionLines : []
        const description = descLines
          .map(l => (l && typeof l.description === 'string' ? l.description.trim() : ''))
          .join('')
        const hasDescription = description !== ''
        const hasName = typeof tag.name === 'string' && tag.name !== ''

        if (tagName === 'param') {
          paramCount++
          if (hasDescription) paramWithDescription++
          if (hasType) paramWithType++
        }

        if (!hasType && !hasName && !hasDescription) {
          emptyTagCount++
        }
      }
    }
  }

  return {
    fixtureDir: path.relative(repoRoot, fixtureDir),
    fileCount: files.length,
    totalLines,
    blockCount,
    paramCount,
    paramWithDescription,
    paramWithType,
    emptyTagCount
  }
}

// ---------------------------------------------------------------------------
// Config generation
// ---------------------------------------------------------------------------

const oxlintCategoriesOff = {
  correctness: 'off',
  nursery: 'off',
  pedantic: 'off',
  perf: 'off',
  restriction: 'off',
  style: 'off',
  suspicious: 'off'
}

/**
 * @param {string} fixtureName
 * @param {{ glob: string, eslintParser: string | null }} fixture
 * @param {string} pattern
 * @param {string[]} rules
 * @param {string} ruleSetName
 */
function generateConfig(fixtureName, fixture, pattern, rules, ruleSetName) {
  const dir = path.join(tmpRoot, fixtureName, pattern, ruleSetName)
  mkdirSync(dir, { recursive: true })

  // Build the ESLint flat config block. TS fixtures need an explicit
  // parser via `languageOptions.parser`; JS fixtures use ESLint's
  // default espree parser.
  const parserImport = fixture.eslintParser
    ? `import tsParser from '${fixture.eslintParser}';\n`
    : ''
  const parserConfig = fixture.eslintParser
    ? `    languageOptions: { parser: tsParser },\n`
    : ''

  // ESLint linterOptions: skip inline /* eslint-disable */ directives so
  // the fixture's many disable comments referencing unrelated rules
  // don't pollute the diagnostic load with ESLint's inline-config
  // bookkeeping rather than the JSDoc rule itself.
  const linterOptions =
    `    linterOptions: { noInlineConfig: true, reportUnusedDisableDirectives: 'off' },\n`

  if (pattern === 'eslint-jsdoc-upstream') {
    const ruleEntries = rules.map(r => `      'jsdoc/${r}': 'error'`).join(',\n')
    const config = `// auto-generated by jsdoc-linter-setup.mjs
import jsdoc from 'eslint-plugin-jsdoc';
${parserImport}export default [
  {
    files: ['${fixture.glob}'],
${parserConfig}    plugins: { jsdoc },
${linterOptions}    rules: {
${ruleEntries}
    }
  }
];
`
    writeFileSync(path.join(dir, 'eslint.config.js'), config)
    return
  }

  if (pattern === 'eslint-ox-jsdoc-single' || pattern === 'eslint-ox-jsdoc-batch') {
    const strategy = pattern === 'eslint-ox-jsdoc-batch' ? 'batch' : 'single'
    const ruleEntries = rules.map(r => `      'jsdoc/${r}': 'error'`).join(',\n')
    const config = `// auto-generated by jsdoc-linter-setup.mjs
import jsdoc from '@ox-jsdoc/eslint-plugin-jsdoc';
${parserImport}export default [
  {
    files: ['${fixture.glob}'],
${parserConfig}    plugins: { jsdoc },
    settings: { jsdoc: { oxParseStrategy: '${strategy}' } },
${linterOptions}    rules: {
${ruleEntries}
    }
  }
];
`
    writeFileSync(path.join(dir, 'eslint.config.js'), config)
    return
  }

  if (pattern === 'oxlint-jsdoc-native') {
    const config = {
      plugins: ['jsdoc'],
      categories: oxlintCategoriesOff,
      rules: Object.fromEntries(rules.map(r => [`jsdoc/${r}`, 'error']))
    }
    writeFileSync(path.join(dir, '.oxlintrc.json'), JSON.stringify(config, null, 2))
    return
  }

  if (pattern === 'oxlint-ox-jsdoc-batch') {
    const config = {
      jsPlugins: [{ name: 'jsdoc-js', specifier: '@ox-jsdoc/eslint-plugin-jsdoc' }],
      settings: { jsdoc: { oxParseStrategy: 'batch' } },
      categories: oxlintCategoriesOff,
      rules: Object.fromEntries(rules.map(r => [`jsdoc-js/${r}`, 'error']))
    }
    writeFileSync(path.join(dir, '.oxlintrc.json'), JSON.stringify(config, null, 2))
    return
  }

  throw new Error(`Unknown pattern: ${pattern}`)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

console.log('Computing fixture stats...')
/** @type {Record<string, ReturnType<typeof computeFixtureStats>>} */
const stats = {}
for (const [name, f] of Object.entries(FIXTURES)) {
  console.log(`\n[${name}] ${path.relative(repoRoot, f.dir)}`)
  stats[name] = computeFixtureStats(f.dir, f.extensions)
  console.log(JSON.stringify(stats[name], null, 2))
}

console.log('\nClearing tmp dir...')
rmSync(tmpRoot, { recursive: true, force: true })
mkdirSync(tmpRoot, { recursive: true })

console.log('\nGenerating configs...')
for (const [fixtureName, fixture] of Object.entries(FIXTURES)) {
  for (const ruleSetName of Object.keys(RULE_SETS)) {
    const rules = RULE_SETS[ruleSetName]
    for (const pattern of PATTERNS) {
      generateConfig(fixtureName, fixture, pattern, rules, ruleSetName)
      console.log(`  ✓ ${fixtureName}/${pattern}/${ruleSetName}`)
    }
  }
}

writeFileSync(path.join(tmpRoot, 'fixture-stats.json'), JSON.stringify(stats, null, 2))
console.log(`\n→ ${path.relative(repoRoot, path.join(tmpRoot, 'fixture-stats.json'))}`)
console.log('Done. Now run: bash tasks/benchmark/scripts/jsdoc-linter-hyperfine.sh')
