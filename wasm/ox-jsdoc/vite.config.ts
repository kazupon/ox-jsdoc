import { defineConfig } from 'vite-plus'
import { playwright } from 'vite-plus/test/browser-playwright'

export default defineConfig({
  test: {
    projects: [
      {
        test: {
          name: 'browser',
          include: ['test/*.test.ts'],
          browser: {
            enabled: true,
            headless: true,
            provider: playwright(),
            instances: [{ browser: 'chromium' }]
          }
        }
      },
      {
        // initWasm() works differently in Node.js: its default location is a
        // file: URL that fetch cannot read, so the .wasm file is passed as bytes.
        test: {
          name: 'node',
          include: ['test/node/*.test.ts'],
          environment: 'node'
        }
      }
    ]
  }
})
