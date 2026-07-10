import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const frontendDirectory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const distDirectory = path.join(frontendDirectory, 'dist')
const assetsDirectory = path.join(distDirectory, 'assets')

const [indexHtml, assetNames] = await Promise.all([
  readFile(path.join(distDirectory, 'index.html'), 'utf8'),
  readdir(assetsDirectory),
])

const cssNames = assetNames.filter((name) => name.endsWith('.css'))
const css = (
  await Promise.all(cssNames.map((name) => readFile(path.join(assetsDirectory, name), 'utf8')))
).join('\n')

const failures = []

if (cssNames.length === 0) failures.push('未生成 CSS 产物')
if (css.includes('@layer')) failures.push('CSS 产物仍包含旧 WebView 不支持的 @layer')
if (!css.includes('--tw-border-style:solid')) {
  failures.push('CSS 产物缺少 @property 的 --tw-border-style 普通变量兜底')
}
if (!css.includes('--background:#fff') || !css.includes('--foreground:#0a0a0a')) {
  failures.push('CSS 产物缺少根主题的传统颜色兜底')
}
if (!css.includes('min-height:100vh') || !/(?:^|[;{])height:100vh/.test(css)) {
  failures.push('CSS 产物缺少 100svh 对应的 100vh 高度兜底')
}
if (!indexHtml.includes('nomodule') || !indexHtml.includes('vite-legacy-entry')) {
  failures.push('index.html 缺少 legacy nomodule 入口')
}
if (!assetNames.some((name) => name.includes('-legacy-') && name.endsWith('.js'))) {
  failures.push('未生成 legacy JavaScript 产物')
}

if (failures.length > 0) {
  throw new Error(`钉钉 legacy 构建检查失败:\n- ${failures.join('\n- ')}`)
}

console.log('钉钉 legacy 构建检查通过')
