import { defineConfig } from 'vite-plus'

export default defineConfig({
  pack: {
    entry: 'src/index.ts',
    outDir: 'dist',
    format: 'es'
  },
  test: {
    include: ['test/**/*.test.ts']
  }
})
