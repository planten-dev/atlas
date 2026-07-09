import path from 'node:path'
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import legacy from '@vitejs/plugin-legacy'
import { tanstackRouter } from '@tanstack/router-plugin/vite'

export default defineConfig({
  plugins: [
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    react(),
    // Tailwind 改走 postcss.config.mjs(@tailwindcss/postcss):
    // 需要串联 cascade-layers 拍平和 @property 兜底,@tailwindcss/vite 不支持挂 PostCSS 插件
    legacy({
      // 钉钉 Android 容器是 UC U4 内核(约 Chromium 69),iOS 是 WKWebView;
      // 这些达不到 Vite 8 的 modern baseline(import.meta.resolve),会走 legacy 构建。
      // Vite 8 不支持降到 ES5,此 targets 下限已是 ES2017+,不受影响。
      targets: ['defaults', 'chrome >= 69', 'android >= 69', 'ios >= 13'],
      // modern 构建也按使用情况注入 core-js polyfill,
      // 覆盖"够新但缺个别 API"的 WebView(如较老的 WKWebView)。
      modernPolyfills: true,
    }),
  ],
  build: {
    // CSS 压缩/降级目标对齐钉钉 Android 容器(UC U4 ≈ Chromium 69),
    // 防止 Lightning CSS 在压缩时把降级写法折叠回新语法
    cssTarget: 'chrome69',
  },
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
