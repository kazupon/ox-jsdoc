import { defineConfig } from 'vite-plus'
import {
  defineFmtConfig,
  defineLintConfig,
  defaultIgnoreFilesOfEnforceHeaderCommentRule
} from '@kazupon/vp-config'

const ignorePatterns = [
  '**/dist/**',
  'crates/**',
  'napi/ox-jsdoc/src-js/binding.*',
  'packages/jsdoccomment/**',
  'packages/eslint-plugin-jsdoc/**',
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
  // @ts-expect-error -- TODO(vp-config): The type of `lint` is not correctly inferred. It should be `LintConfig` instead of `LintConfig[]`.
  lint: defineLintConfig({
    ignorePatterns,
    comments: {
      enForceHeaderComment: {
        ignoreFiles: [...defaultIgnoreFilesOfEnforceHeaderCommentRule]
      }
    }
  })
})
