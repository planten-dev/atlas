import path from 'node:path'
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'

export default defineConfig({
  plugins: [
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      '@': path.resolve(import.meta.dirname, 'src'),
    },
  },
  server: {
    // 显式绑 127.0.0.1:钉钉回调落 cookie 在主机 127.0.0.1 上,
    // 前端必须以同一主机名访问,否则登录后 cookie 带不过去(见 README)。
    host: '127.0.0.1',
    // 后端 frontend_callback_url 写死 5173;端口被占时宁可报错也不静默换端口
    strictPort: true,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:3000',
        changeOrigin: false,
      },
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    globals: true,
  },
})
