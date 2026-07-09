import { createFileRoute, redirect } from '@tanstack/react-router'
import { z } from 'zod'
import { Button } from '@/components/ui/button'
import { login, meQueryOptions } from '@/auth/session'

export const Route = createFileRoute('/login')({
  validateSearch: z.object({
    redirect: z.string().optional(),
  }),
  beforeLoad: async ({ context, search }) => {
    // 已登录直接进应用
    try {
      await context.queryClient.ensureQueryData(meQueryOptions)
      throw redirect({ to: search.redirect ?? '/' })
    } catch (error) {
      if (error && typeof error === 'object' && 'isRedirect' in error) throw error
      // 401:留在登录页
    }
  },
  component: LoginPage,
})

function LoginPage() {
  return (
    <div className="flex min-h-svh flex-col items-center justify-center gap-8 bg-background px-6">
      <div className="flex flex-col items-center gap-2">
        <div className="flex size-16 items-center justify-center rounded-2xl bg-primary text-3xl font-bold text-primary-foreground">
          A
        </div>
        <h1 className="text-2xl font-semibold tracking-tight">Atlas</h1>
        <p className="text-sm text-muted-foreground">销售与耗用管理系统</p>
      </div>
      <Button size="lg" className="w-full max-w-xs" onClick={login}>
        使用钉钉登录
      </Button>
    </div>
  )
}
