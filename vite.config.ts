import { defineConfig } from 'vite-plus'
import {
  defineFmtConfig,
  defineLintConfig,
  defaultIgnoreFilesOfEnforceHeaderCommentRule
} from '@kazupon/vp-config'

export default defineConfig({
  staged: {
    '*': 'vp check --fix'
  },
  test: {
    include: ['napi/**/*.test.ts']
  },
  fmt: defineFmtConfig({
    ignorePatterns: [
      'crates/**',
      'napi/ox-jsdoc/src-js/binding.*',
      'tasks/**',
      'refers/**',
      'fixtures/**'
    ]
  }),
  // @ts-expect-error -- FIXME: The type of `lint` is not correctly inferred. It should be `LintConfig` instead of `LintConfig[]`.
  lint: defineLintConfig({
    ignorePatterns: [
      'crates/**',
      'napi/ox-jsdoc/src-js/binding.*',
      'tasks/**',
      'refers/**',
      'fixtures/**'
    ],
    comments: {
      enForceHeaderComment: {
        ignoreFiles: [...defaultIgnoreFilesOfEnforceHeaderCommentRule]
      }
    }
  })
})
