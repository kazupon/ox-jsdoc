import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'
import { voidPlugin } from 'void'

export default defineConfig({
  plugins: [voidPlugin(), vue()]
})
