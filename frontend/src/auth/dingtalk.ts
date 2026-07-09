/** 钉钉容器检测与免登 authCode 获取(H5 微应用)。 */

export function isDingTalkWebview(ua: string = navigator.userAgent): boolean {
  return /DingTalk/i.test(ua)
}

/**
 * 在钉钉容器内取免登 authCode(需 corpId)。
 * SDK 动态加载:仅钉钉路径引入,不进主包。
 */
export async function fetchDingTalkAuthCode(corpId: string): Promise<string> {
  const dd = (await import('dingtalk-jsapi')).default
  const result = await dd.runtime.permission.requestAuthCode({ corpId })
  if (!result?.code) {
    throw new Error('DingTalk requestAuthCode returned no code')
  }
  return result.code
}
