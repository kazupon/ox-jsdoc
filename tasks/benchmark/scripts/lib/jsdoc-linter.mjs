/**
 * The fixtures, rule sets and patterns of the JSDoc linter benchmark, shared
 * by `jsdoc-linter-setup.mjs`, `jsdoc-linter-diagnostics.mjs` and
 * `jsdoc-linter-report.mjs`.
 */

import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

export const repoRoot = path.resolve(__dirname, '../../../..')
export const benchRoot = path.resolve(__dirname, '../..')
export const tmpRoot = path.join(benchRoot, '.tmp/jsdoc-linter')
export const resultsRoot = path.join(benchRoot, 'results')

/**
 * Two fixture variants — one JS, one TS. They share the rule sets and
 * pattern matrix but differ in extension glob and (for ESLint) parser. Both
 * linters apply the rules to the files of `glob` only, so they do the same
 * work.
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

/**
 * The patterns that run the same rules of `eslint-plugin-jsdoc` (upstream,
 * or the fork with another parser), so they must report the same
 * diagnostics. `oxlint-jsdoc-native` is another implementation of the rules
 * and is a reference only.
 */
export const SAME_RULES_PATTERNS = [
  'eslint-jsdoc-upstream',
  'eslint-ox-jsdoc-single',
  'eslint-ox-jsdoc-batch',
  'oxlint-ox-jsdoc-batch'
]
