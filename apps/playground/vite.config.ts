import vue from '@vitejs/plugin-vue'
import { createReadStream, readFileSync } from 'node:fs'
import { basename, join } from 'node:path'
import { defineConfig, type Plugin } from 'vite'
import { voidPlugin } from 'void'

const oxJsdocWasmAssetNames = ['ox_jsdoc_wasm_bg.wasm']

function getContentType(filename: string): string {
  if (filename.endsWith('.wasm')) {
    return 'application/wasm'
  }

  return 'text/javascript; charset=utf-8'
}

function oxJsdocWasmAssets(): Plugin {
  const assetDir = join(process.cwd(), '..', '..', 'wasm', 'ox-jsdoc', 'pkg')

  return {
    name: 'ox-jsdoc-wasm-assets',
    configureServer(server) {
      server.middlewares.use('/vendor/ox-jsdoc', (request, response, next) => {
        const pathname = decodeURIComponent((request.url ?? '').split('?')[0] ?? '')
        const filename = basename(pathname)

        if (!oxJsdocWasmAssetNames.includes(filename)) {
          next()
          return
        }

        response.setHeader('Content-Type', getContentType(filename))
        createReadStream(join(assetDir, filename)).pipe(response)
      })
    },
    async generateBundle() {
      for (const filename of oxJsdocWasmAssetNames) {
        this.emitFile({
          type: 'asset',
          fileName: `vendor/ox-jsdoc/${filename}`,
          source: readFileSync(join(assetDir, filename))
        })
      }
    }
  }
}

export default defineConfig({
  build: {
    chunkSizeWarningLimit: 4000
  },
  plugins: [oxJsdocWasmAssets(), voidPlugin(), vue()]
})
