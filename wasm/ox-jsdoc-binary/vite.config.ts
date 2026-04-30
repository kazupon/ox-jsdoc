import { defineConfig } from 'vite-plus'
import { playwright } from 'vite-plus/test/browser-playwright'

export default defineConfig({
  test: {
    include: ['test/**/*.test.ts'],
    browser: {
      enabled: true,
      headless: true,
      // @ts-expect-error -- FIXME(playwright): The `provider` option is currently not typed in `vite-plus`'s test configuration, but it should be.
      provider: playwright(),
      instances: [{ browser: 'chromium' }]
    }
  }
})
