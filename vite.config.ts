import { defineConfig } from 'vite-plus'
import {
  defineFmtConfig,
  defineLintConfig,
  defaultIgnoreFilesOfEnforceHeaderCommentRule,
  defaultVitestTargetFiles
} from '@kazupon/vp-config'

const ignorePatterns = [
  '**/dist/**',
  'crates/**',
  'CHANGELOG.md',
  'napi/*/src-js/bindings.*',
  'napi/*/src-js/*.node',
  'wasm/*/pkg/**',
  'packages/jsdoccomment/**',
  'packages/eslint-plugin-jsdoc/**',
  'apps/playground/public/vendor/**',
  'tasks/**',
  'scripts/**',
  'refers/**',
  'fixtures/**'
]

export default defineConfig({
  staged: {
    '*': 'vp check --fix'
  },
  fmt: defineFmtConfig({
    ignorePatterns
  }),
  lint: defineLintConfig({
    ignorePatterns,
    rules: {
      // JSDoc comments write the closing delimiter as `*`, U+200B (zero-width space), `/`
      // so that it does not end the comment. Check strings and code only.
      'no-irregular-whitespace': ['warn', { skipComments: true }]
    },
    overrides: [
      {
        files: defaultVitestTargetFiles,
        plugins: ['vitest'],
        rules: {
          // `expectAccepts()` in napi/ox-jsdoc/test/parsed-type.test.ts wraps `expect`
          'vitest/expect-expect': ['warn', { assertFunctionNames: ['expect', 'expectAccepts'] }]
        }
      }
    ],
    comments: {
      enForceHeaderComment: {
        ignoreFiles: [...defaultIgnoreFilesOfEnforceHeaderCommentRule]
      }
    }
  })
})
