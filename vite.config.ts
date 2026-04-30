import { defineConfig } from 'vite-plus'
import { playwright } from 'vite-plus/test/browser-playwright'
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
  'tasks/**',
  'scripts/**',
  'refers/**',
  'fixtures/**'
]

export default defineConfig({
  staged: {
    '*': 'vp check --fix'
  },
  test: {
    projects: [
      // NOTE(kazupon): napi binding tests cannot run in the root config due to vitest's module runner limitations. See `napi/ox-jsdoc/vitest.config.ts` for details.
      // {
      //   test: {
      //     name: 'napi',
      //     include: ['napi/**/*.test.ts'],
      //     environment: 'node'
      //   }
      // },
      {
        test: {
          name: 'decoder',
          include: ['packages/decoder/test/**/*.test.js']
        }
      },
      {
        test: {
          name: 'wasm',
          include: ['wasm/**/*.test.ts'],
          browser: {
            enabled: true,
            headless: true,
            provider: playwright() as any, // NOTE(vitest): The type of `provider` is not correctly inferred. It should be `PlaywrightProvider` instead of `BrowserProvider`.
            instances: [{ browser: 'chromium' }]
          }
        }
      }
    ]
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
