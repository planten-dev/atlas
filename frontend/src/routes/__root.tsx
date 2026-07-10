import type { QueryClient } from '@tanstack/react-query'
import { Outlet, createRootRouteWithContext, Link } from '@tanstack/react-router'

export interface RouterContext {
  queryClient: QueryClient
}

declare module '@tanstack/react-router' {
  interface StaticDataRouteOption {
    /** 移动底部 TabBar 的归属 tab;仅标记了 tab 的路由显示 TabBar。 */
    tab?: 'home' | 'sales' | 'approvals' | 'me'
    /** 桌面专属页面:移动端显示"请到电脑端操作"提示。 */
    desktopOnly?: boolean
  }
}

function NotFound() {
  return (
    <div className="min-h-app flex flex-col items-center justify-center gap-4">
      <p className="text-5xl font-bold text-muted-foreground">404</p>
      <p className="text-muted-foreground">页面不存在</p>
      <Link to="/" className="text-primary underline underline-offset-4">
        回到工作台
      </Link>
    </div>
  )
}

function RootError({ error }: { error: Error }) {
  return (
    <div className="min-h-app flex flex-col items-center justify-center gap-4 px-6 text-center">
      <p className="text-xl font-semibold">页面出错了</p>
      <p className="max-w-md break-all text-sm text-muted-foreground">{error.message}</p>
      <a href="/" className="text-primary underline underline-offset-4">
        回到工作台
      </a>
    </div>
  )
}

export const Route = createRootRouteWithContext<RouterContext>()({
  component: () => <Outlet />,
  notFoundComponent: NotFound,
  errorComponent: RootError,
})
