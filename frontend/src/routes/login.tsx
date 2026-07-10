import { useEffect, useState } from 'react'
import { createFileRoute, redirect, useNavigate } from '@tanstack/react-router'
import { useQueryClient } from '@tanstack/react-query'
import { z } from 'zod'
import { Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { login, loginWithDingTalkH5, meQueryOptions } from '@/auth/session'
import {
  createDingTalkH5AttemptTracker,
  fetchDingTalkAuthCode,
  isDingTalkWebview,
} from '@/auth/dingtalk'
import { notify } from '@/lib/notify'

const h5AttemptTracker = createDingTalkH5AttemptTracker()

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

function shouldAttemptH5Login(): boolean {
  const corpId = import.meta.env.VITE_DINGTALK_CORP_ID as string | undefined
  return Boolean(corpId) && isDingTalkWebview() && !h5AttemptTracker.hasAttempted()
}

function LoginPage() {
  const search = Route.useSearch()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  // 钉钉容器内自动免登:挂载时一次性判定;sessionStorage 标记防 authCode 重试风暴
  const [h5Pending, setH5Pending] = useState(shouldAttemptH5Login)

  useEffect(() => {
    if (!h5Pending || h5AttemptTracker.hasAttempted()) return
    h5AttemptTracker.markAttempted()
    const corpId = import.meta.env.VITE_DINGTALK_CORP_ID as string
    void (async () => {
      try {
        const authCode = await fetchDingTalkAuthCode(corpId)
        await loginWithDingTalkH5(authCode)
        await queryClient.invalidateQueries({ queryKey: ['auth'] })
        void navigate({ to: search.redirect ?? '/' })
      } catch {
        notify.error('钉钉自动登录失败，请使用按钮重新登录')
        setH5Pending(false)
      }
    })()
  }, [h5Pending, navigate, queryClient, search.redirect])

  return (
    <div className="min-h-app flex flex-col items-center justify-center gap-8 bg-background px-6">
      <div className="flex flex-col items-center gap-2">
        <div className="flex size-16 items-center justify-center rounded-2xl bg-primary text-3xl font-bold text-primary-foreground">
          A
        </div>
        <h1 className="text-2xl font-semibold tracking-tight">Atlas</h1>
        <p className="text-sm text-muted-foreground">销售与耗用管理系统</p>
      </div>
      {h5Pending ? (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" />
          钉钉自动登录中…
        </div>
      ) : (
        <Button size="lg" className="w-full max-w-xs" onClick={login}>
          使用钉钉登录
        </Button>
      )}
    </div>
  )
}
