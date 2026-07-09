import createClient, { type Middleware } from 'openapi-fetch'
import type { paths } from '@/api/types.gen'
import { ApiError } from '@/api/error'

/**
 * 全应用唯一 fetch 封装(设计 §3):
 * - openapi.json 的路径自带 /api/v1 前缀,故 baseUrl 为空(同源,dev 由 Vite proxy 转发)。
 * - 会话为 HttpOnly cookie(atlas_session),前端不管理任何 token。
 * - 非 2xx 统一抛 ApiError;401 先通知全局回调(清缓存跳登录)。
 */

type UnauthorizedHandler = (currentPath: string) => void

let onUnauthorized: UnauthorizedHandler | null = null

export function setUnauthorizedHandler(handler: UnauthorizedHandler): void {
  onUnauthorized = handler
}

/** 带该 header 的请求 401 时不触发全局跳转(用于启动引导的 /auth/me 探测)。 */
export const SKIP_AUTH_REDIRECT_HEADER = 'x-skip-auth-redirect'

const errorMiddleware: Middleware = {
  async onResponse({ request, response }) {
    if (response.ok) return response

    let code = 'unknown_error'
    let message = `请求失败(${response.status})`
    try {
      const body = (await response.clone().json()) as { error?: string; message?: string }
      if (body.error) code = body.error
      if (body.message) message = body.message
    } catch {
      // 非 JSON 错误体,保留默认文案
    }

    if (response.status === 401 && !request.headers.has(SKIP_AUTH_REDIRECT_HEADER)) {
      onUnauthorized?.(window.location.pathname + window.location.search)
    }

    throw new ApiError(response.status, code, message)
  },
}

// 同源(openapi 路径自带 /api/v1)。显式用 origin:Node/jsdom 的 fetch 不接受相对 URL。
// fetch 延迟解析:MSW 在 listen() 时才替换全局 fetch,提前捕获会绕过测试拦截。
export const client = createClient<paths>({
  baseUrl: typeof window === 'undefined' ? '' : window.location.origin,
  fetch: (input) => globalThis.fetch(input),
})
client.use(errorMiddleware)

/** openapi-fetch 在中间件抛错后不会返回 data,这里统一 unwrap。 */
export function unwrap<T>(result: { data?: T }): T {
  if (result.data === undefined) {
    throw new ApiError(500, 'empty_response', '响应为空')
  }
  return result.data
}
