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
  fmt: defineFmtConfig({
    ignorePatterns: ['crates/**', 'tasks/**', 'refers/**']
  }),
  // @ts-expect-error -- FIXME: The type of `lint` is not correctly inferred. It should be `LintConfig` instead of `LintConfig[]`.
  lint: defineLintConfig({
    ignorePatterns: ['crates/**', 'tasks/**', 'refers/**'],
    comments: {
      enForceHeaderComment: {
        ignoreFiles: [...defaultIgnoreFilesOfEnforceHeaderCommentRule]
      }
    }
  })
})
