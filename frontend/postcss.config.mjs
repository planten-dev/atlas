import tailwindcss from '@tailwindcss/postcss'
import postcssCascadeLayers from '@csstools/postcss-cascade-layers'

/**
 * 老 Chromium 会忽略 @property，导致 Tailwind 的 --tw-* 初始值缺失。
 * 将注册属性的 initial-value 同步为普通自定义属性，现代浏览器仍保留原注册。
 */
const propertyInitialFallback = {
  postcssPlugin: 'atlas-property-initial-fallback',
  OnceExit(root, { Rule, Declaration }) {
    const fallbacks = []

    root.walkAtRules('property', (atRule) => {
      atRule.walkDecls('initial-value', (declaration) => {
        fallbacks.push([atRule.params.trim(), declaration.value])
      })
    })

    if (fallbacks.length === 0) return

    const rule = new Rule({ selector: '*, ::before, ::after, ::backdrop' })
    for (const [property, value] of fallbacks) {
      rule.append(new Declaration({ prop: property, value }))
    }
    root.append(rule)
  },
}

export default {
  plugins: [tailwindcss(), postcssCascadeLayers(), propertyInitialFallback],
}
