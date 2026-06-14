import { defineConfig } from 'vite-plus'
import {
  defineFmtConfig,
  defineLintConfig,
  defaultIgnoreFilesOfEnforceHeaderCommentRule
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
    comments: {
      enForceHeaderComment: {
        ignoreFiles: [...defaultIgnoreFilesOfEnforceHeaderCommentRule]
      }
    }
  })
})
