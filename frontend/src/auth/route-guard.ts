import { redirect } from '@tanstack/react-router'
import type { QueryClient } from '@tanstack/react-query'
import { myPermissionsQueryOptions } from '@/auth/session'

/** 路由 beforeLoad 权限守卫:缺权限重定向 403 页。 */
export async function requirePerm(queryClient: QueryClient, perm: string): Promise<void> {
  const permissions = await queryClient.ensureQueryData(myPermissionsQueryOptions)
  if (!permissions.has(perm)) {
    throw redirect({ to: '/403' })
  }
}
