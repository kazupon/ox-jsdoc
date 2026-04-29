import { configDefaults, defineConfig } from 'vite-plus'

export default defineConfig({
  test: {
    exclude: [...configDefaults.exclude],
    include: ['./test/**/*.test.js'],
    coverage: {
      include: ['src/**'],
      exclude: ['test/**'],
      provider: 'v8',
      reporter: ['text', 'json', 'html']
    },
    reporters: ['default'],
    globals: true
  }
})
