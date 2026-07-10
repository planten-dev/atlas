import { describe, expect, it, vi } from 'vitest'
import { prepareBrowserCompatibility, renderUnsupportedBrowser } from './browser'

function createBrowserWindow({
  supportsModernCss = true,
  hasResizeObserver = true,
}: {
  supportsModernCss?: boolean
  hasResizeObserver?: boolean
} = {}) {
  const browserWindow = {
    navigator: { userAgent: 'Mozilla/5.0 AliApp(DingTalk/7.5.0)' },
    fetch: vi.fn(),
    matchMedia: vi.fn(),
    CSS: {
      supports: vi.fn((propertyOrCondition: string, value?: string) => {
        if (propertyOrCondition === 'selector(:has(*))') return supportsModernCss
        if (propertyOrCondition === 'height' && value === '100svh') return supportsModernCss
        return true
      }),
    },
  } as unknown as Window & typeof globalThis

  if (hasResizeObserver) {
    Object.defineProperty(browserWindow, 'ResizeObserver', {
      configurable: true,
      writable: true,
      value: class {},
    })
  }
  return browserWindow
}

describe('prepareBrowserCompatibility', () => {
  it('标记钉钉和 legacy WebView', async () => {
    const browserDocument = document.implementation.createHTMLDocument()
    const result = await prepareBrowserCompatibility({
      browserWindow: createBrowserWindow({ supportsModernCss: false }),
      browserDocument,
    })

    expect(result).toEqual({ supported: true, legacy: true })
    expect(browserDocument.documentElement.classList.contains('dingtalk-webview')).toBe(true)
    expect(browserDocument.documentElement.classList.contains('legacy-webview')).toBe(true)
  })

  it('缺少 ResizeObserver 时按需安装 polyfill', async () => {
    const browserWindow = createBrowserWindow({ hasResizeObserver: false })
    const ResizeObserverPolyfill = class {}

    const result = await prepareBrowserCompatibility({
      browserWindow,
      browserDocument: document.implementation.createHTMLDocument(),
      loadResizeObserver: async () =>
        ResizeObserverPolyfill as unknown as typeof ResizeObserver,
    })

    expect(result).toEqual({ supported: true, legacy: false })
    expect(browserWindow.ResizeObserver).toBe(ResizeObserverPolyfill)
  })

  it('polyfill 加载失败时阻止应用启动', async () => {
    const result = await prepareBrowserCompatibility({
      browserWindow: createBrowserWindow({ hasResizeObserver: false }),
      browserDocument: document.implementation.createHTMLDocument(),
      loadResizeObserver: async () => {
        throw new Error('load failed')
      },
    })

    expect(result).toEqual({
      supported: false,
      reason: 'resize-observer-polyfill-failed',
    })
  })

  it('缺少 fetch 时返回明确原因', async () => {
    const browserWindow = createBrowserWindow()
    Object.defineProperty(browserWindow, 'fetch', { value: undefined })

    await expect(
      prepareBrowserCompatibility({
        browserWindow,
        browserDocument: document.implementation.createHTMLDocument(),
      }),
    ).resolves.toEqual({ supported: false, reason: 'missing-fetch' })
  })
})

describe('renderUnsupportedBrowser', () => {
  it('输出不依赖应用组件的升级提示', () => {
    const root = document.createElement('div')
    renderUnsupportedBrowser(root)

    expect(root).toHaveTextContent('当前钉钉版本暂不受支持')
    expect(root.querySelector('button')).toHaveTextContent('重新加载')
  })
})
