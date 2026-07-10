/** 钉钉容器检测与免登 authCode 获取(H5 微应用)。 */

export const DINGTALK_H5_ATTEMPT_KEY = 'atlas-dd-h5-login-attempted'

interface StorageLike {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

export interface DingTalkH5AttemptTracker {
  hasAttempted(): boolean
  markAttempted(): void
}

/** sessionStorage 不可用时退回内存标记，仍能阻止 StrictMode/刷新风暴。 */
export function createDingTalkH5AttemptTracker(
  getStorage: () => StorageLike | null = () => window.sessionStorage,
): DingTalkH5AttemptTracker {
  let attemptedInMemory = false

  return {
    hasAttempted() {
      if (attemptedInMemory) return true
      try {
        return Boolean(getStorage()?.getItem(DINGTALK_H5_ATTEMPT_KEY))
      } catch {
        return false
      }
    },
    markAttempted() {
      attemptedInMemory = true
      try {
        getStorage()?.setItem(DINGTALK_H5_ATTEMPT_KEY, '1')
      } catch {
        // WebView 隐私策略可能禁用 storage；内存标记已足够保护当前页面生命周期。
      }
    },
  }
}

export function isDingTalkWebview(ua: string = navigator.userAgent): boolean {
  return /DingTalk/i.test(ua)
}

/**
 * 在钉钉容器内取免登 authCode(需 corpId)。
 * SDK 动态加载:仅钉钉路径引入,不进主包。
 */
interface AuthCodeResult {
  code?: string
}

interface FetchDingTalkAuthCodeOptions {
  timeoutMs?: number
  requestAuthCode?: (corpId: string) => Promise<AuthCodeResult>
}

async function requestAuthCodeWithSdk(corpId: string): Promise<AuthCodeResult> {
  const dd = (await import('dingtalk-jsapi')).default
  return dd.runtime.permission.requestAuthCode({ corpId })
}

export async function fetchDingTalkAuthCode(
  corpId: string,
  options: FetchDingTalkAuthCodeOptions = {},
): Promise<string> {
  const timeoutMs = options.timeoutMs ?? 10_000
  const requestAuthCode = options.requestAuthCode ?? requestAuthCodeWithSdk
  let timeout: ReturnType<typeof setTimeout> | undefined

  try {
    const result = await Promise.race([
      requestAuthCode(corpId),
      new Promise<never>((_, reject) => {
        timeout = setTimeout(
          () => reject(new Error('DingTalk requestAuthCode timed out')),
          timeoutMs,
        )
      }),
    ])

    if (!result?.code) {
      throw new Error('DingTalk requestAuthCode returned no code')
    }
    return result.code
  } finally {
    if (timeout !== undefined) clearTimeout(timeout)
  }
}
