import { queryOptions } from '@tanstack/react-query'
import { client, unwrap, SKIP_AUTH_REDIRECT_HEADER } from '@/api/client'

export const meQueryOptions = queryOptions({
  queryKey: ['auth', 'me'],
  queryFn: async () =>
    unwrap(
      await client.GET('/api/v1/auth/me', {
        headers: { [SKIP_AUTH_REDIRECT_HEADER]: '1' },
      }),
    ),
  staleTime: 5 * 60_000,
  retry: false,
})

export const myPermissionsQueryOptions = queryOptions({
  queryKey: ['auth', 'permissions'],
  queryFn: async () => {
    const data = unwrap(await client.GET('/api/v1/auth/me/permissions'))
    return new Set(data.permissions)
  },
  staleTime: 5 * 60_000,
})

/** 登录 = 整页跳转到后端 OAuth 入口(设计 §4)。 */
export function login(): void {
  window.location.href = '/api/v1/auth/login/dingtalk'
}

/** 钉钉 H5 免登:JSAPI authCode 换会话 cookie(失败不触发 401 跳转,由登录页兜底)。 */
export async function loginWithDingTalkH5(authCode: string) {
  return unwrap(
    await client.POST('/api/v1/auth/login/dingtalk/h5', {
      body: { authCode },
      headers: { [SKIP_AUTH_REDIRECT_HEADER]: '1' },
    }),
  )
}

export async function logout(): Promise<void> {
  await client.POST('/api/v1/auth/logout')
}
