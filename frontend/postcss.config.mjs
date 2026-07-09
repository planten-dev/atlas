import tailwindcss from '@tailwindcss/postcss'
import postcssCascadeLayers from '@csstools/postcss-cascade-layers'

// 钉钉容器(UC U4 内核/老 WKWebView)不支持 @property。Tailwind 自带的
// 变量兜底块用 -webkit-hyphens/-moz-orient 探测,只命中老 Safari/Firefox,
// 老 Chromium 探测不到 → --tw-border-style 等未定义 → border-style 解析
// 失败回退 none,边框消失。这里把所有 @property 的 initial-value 无条件
// 落成普通自定义属性;现代浏览器上与 @property 初始值等价,无副作用。
const propertyInitialFallback = {
  postcssPlugin: 'property-initial-fallback',
  OnceExit(root, { Rule, Declaration }) {
    const fallbacks = []
    root.walkAtRules('property', (atRule) => {
      atRule.walkDecls('initial-value', (decl) => {
        fallbacks.push([atRule.params.trim(), decl.value])
      })
    })
    if (fallbacks.length === 0) return
    const rule = new Rule({ selector: '*, ::before, ::after, ::backdrop' })
    for (const [prop, value] of fallbacks) {
      rule.append(new Declaration({ prop, value }))
    }
    root.append(rule)
  },
}

export default {
  plugins: [
    tailwindcss(),
    // 拍平 @layer:钉钉容器不支持 cascade layers 时会整层忽略,
    // base 层里的 * { border-color: var(--border) } 失效,边框变 currentColor(黑)
    postcssCascadeLayers(),
    propertyInitialFallback,
  ],
}
