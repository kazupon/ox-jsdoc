import { defineConfig } from 'vite-plus'
import { playwright } from 'vite-plus/test/browser-playwright'
import {
  defineFmtConfig,
  defineLintConfig,
  defaultIgnoreFilesOfEnforceHeaderCommentRule
} from '@kazupon/vp-config'

const ignorePatterns = [
  'crates/**',
  'napi/ox-jsdoc/src-js/binding.*',
  'tasks/**',
  'refers/**',
  'fixtures/**'
]

export default defineConfig({
  staged: {
    '*': 'vp check --fix'
  },
  test: {
    projects: [
      {
        test: {
          name: 'node',
          include: ['napi/**/*.test.ts']
        }
      },
      {
        test: {
          name: 'browser',
          include: ['wasm/**/*.test.ts'],
          browser: {
            enabled: true,
            provider: playwright(),
            headless: true,
            instances: [{ browser: 'chromium' }]
          }
        }
      }
    ]
  },
  fmt: defineFmtConfig({
    ignorePatterns
  }),
  // @ts-expect-error -- FIXME: The type of `lint` is not correctly inferred. It should be `LintConfig` instead of `LintConfig[]`.
  lint: defineLintConfig({
    ignorePatterns,
    comments: {
      enForceHeaderComment: {
        ignoreFiles: [...defaultIgnoreFilesOfEnforceHeaderCommentRule]
      }
    }
  })
})
