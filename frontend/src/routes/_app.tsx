import { Outlet, createFileRoute, redirect } from '@tanstack/react-router'
import { useSuspenseQuery } from '@tanstack/react-query'
import { ApiError } from '@/api/error'
import { meQueryOptions, myPermissionsQueryOptions } from '@/auth/session'
import { PermissionProvider } from '@/auth/PermissionProvider'
import { AppShell } from '@/layouts/AppShell'

export const Route = createFileRoute('/_app')({
  beforeLoad: async ({ context, location }) => {
    try {
      await context.queryClient.ensureQueryData(meQueryOptions)
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        throw redirect({
          to: '/login',
          search: location.pathname !== '/' ? { redirect: location.href } : {},
        })
      }
      throw error
    }
    // 登录成功后并行预取权限集(设计 §4)
    await context.queryClient.ensureQueryData(myPermissionsQueryOptions)
  },
  component: AppLayout,
})

function AppLayout() {
  const { data: permissions } = useSuspenseQuery(myPermissionsQueryOptions)
  return (
    <PermissionProvider permissions={permissions}>
      <AppShell>
        <Outlet />
      </AppShell>
    </PermissionProvider>
  )
}
