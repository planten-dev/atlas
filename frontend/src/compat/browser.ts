import { isDingTalkWebview } from '@/auth/dingtalk'

export type BrowserCompatibilityReason =
  | 'missing-fetch'
  | 'missing-match-media'
  | 'missing-css-supports'
  | 'resize-observer-polyfill-failed'

export type BrowserCompatibilityResult =
  | { supported: true; legacy: boolean }
  | { supported: false; reason: BrowserCompatibilityReason }

type ResizeObserverConstructor = typeof ResizeObserver

interface BrowserCompatibilityOptions {
  browserWindow?: Window & typeof globalThis
  browserDocument?: Document
  userAgent?: string
  loadResizeObserver?: () => Promise<ResizeObserverConstructor>
}

async function loadResizeObserverPolyfill(): Promise<ResizeObserverConstructor> {
  const module = await import('@juggle/resize-observer')
  return module.ResizeObserver as unknown as ResizeObserverConstructor
}

function supportsCss(css: typeof CSS, condition: string): boolean
function supportsCss(css: typeof CSS, property: string, value: string): boolean
function supportsCss(css: typeof CSS, propertyOrCondition: string, value?: string): boolean {
  try {
    return value === undefined
      ? css.supports(propertyOrCondition)
      : css.supports(propertyOrCondition, value)
  } catch {
    return false
  }
}

/** 在 React 挂载前补齐关键 DOM API，并标记需要兼容样式的浏览器。 */
export async function prepareBrowserCompatibility(
  options: BrowserCompatibilityOptions = {},
): Promise<BrowserCompatibilityResult> {
  const browserWindow = options.browserWindow ?? window
  const browserDocument = options.browserDocument ?? document
  const userAgent = options.userAgent ?? browserWindow.navigator.userAgent
  const root = browserDocument.documentElement

  root.classList.toggle('dingtalk-webview', isDingTalkWebview(userAgent))

  if (typeof browserWindow.fetch !== 'function') {
    return { supported: false, reason: 'missing-fetch' }
  }
  if (typeof browserWindow.matchMedia !== 'function') {
    return { supported: false, reason: 'missing-match-media' }
  }
  if (!browserWindow.CSS || typeof browserWindow.CSS.supports !== 'function') {
    return { supported: false, reason: 'missing-css-supports' }
  }

  const legacy =
    !supportsCss(browserWindow.CSS, 'selector(:has(*))') ||
    !supportsCss(browserWindow.CSS, 'height', '100svh')
  root.classList.toggle('legacy-webview', legacy)

  if (typeof browserWindow.ResizeObserver !== 'function') {
    try {
      const ResizeObserverPolyfill = await (
        options.loadResizeObserver ?? loadResizeObserverPolyfill
      )()
      Object.defineProperty(browserWindow, 'ResizeObserver', {
        configurable: true,
        writable: true,
        value: ResizeObserverPolyfill,
      })
    } catch {
      return { supported: false, reason: 'resize-observer-polyfill-failed' }
    }
  }

  return { supported: true, legacy }
}

/** 不依赖应用 CSS/组件库的最终兜底，确保极旧 WebView 不会只显示白屏。 */
export function renderUnsupportedBrowser(root: HTMLElement): void {
  const wrapper = document.createElement('main')
  wrapper.style.cssText =
    'box-sizing:border-box;min-height:100vh;padding:48px 24px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:16px;background:#fff;color:#171717;font-family:system-ui,sans-serif;text-align:center'

  const title = document.createElement('h1')
  title.textContent = '当前钉钉版本暂不受支持'
  title.style.cssText = 'margin:0;font-size:22px;line-height:1.4'

  const description = document.createElement('p')
  description.textContent = '请升级钉钉后重新打开 Atlas。若已升级，可点击下方按钮重试。'
  description.style.cssText = 'margin:0;max-width:420px;color:#525252;font-size:14px;line-height:1.7'

  const retry = document.createElement('button')
  retry.type = 'button'
  retry.textContent = '重新加载'
  retry.style.cssText =
    'border:0;border-radius:8px;padding:10px 18px;background:#171717;color:#fff;font-size:14px;cursor:pointer'
  retry.addEventListener('click', () => window.location.reload())

  wrapper.append(title, description, retry)
  root.textContent = ''
  root.appendChild(wrapper)
}
