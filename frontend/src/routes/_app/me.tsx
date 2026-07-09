import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useMutation, useQueryClient, useSuspenseQuery } from '@tanstack/react-query'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { logout, meQueryOptions } from '@/auth/session'
import { usePermissions } from '@/auth/PermissionProvider'
import { useIsMobile } from '@/hooks/use-mobile'
import { useUserProfile } from '@/hooks/useUserProfile'
import { formatDateTime } from '@/lib/date'
import { notify } from '@/lib/notify'

export const Route = createFileRoute('/_app/me')({
  staticData: { tab: 'me' },
  component: MePage,
})

function MePage() {
  const { data: me } = useSuspenseQuery(meQueryOptions)
  const { data: profile } = useUserProfile(me.id)
  const permissions = usePermissions()
  const isMobile = useIsMobile()
  const queryClient = useQueryClient()
  const navigate = useNavigate()

  const logoutMutation = useMutation({
    mutationFn: logout,
    onSuccess: () => {
      queryClient.clear()
      void navigate({ to: '/login' })
    },
    onError: (error) => notify.error(error),
  })

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4">
      <h1 className="text-xl font-semibold">个人中心</h1>

      <Card>
        <CardContent className="flex items-center gap-4 py-4">
          <Avatar className="size-14">
            <AvatarImage src={profile?.avatar_url ?? undefined} />
            <AvatarFallback>{profile?.name?.slice(0, 1) ?? '?'}</AvatarFallback>
          </Avatar>
          <div className="flex flex-col gap-0.5">
            <p className="font-medium">{profile?.name ?? me.dingtalk_user_id}</p>
            {profile?.title && <p className="text-sm text-muted-foreground">{profile.title}</p>}
            <p className="text-xs text-muted-foreground">
              上次登录:{formatDateTime(me.last_login_at)}
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">我的权限</CardTitle>
        </CardHeader>
        <CardContent>
          {permissions.size === 0 ? (
            <p className="text-sm text-muted-foreground">暂无权限,请联系管理员</p>
          ) : (
            <div className="flex flex-wrap gap-1.5">
              {[...permissions].sort().map((perm) => (
                <Badge key={perm} variant="secondary">
                  {perm}
                </Badge>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* 桌面端退出走右上角头像菜单;移动端无头像菜单,保留此入口(设计 §7) */}
      {isMobile && (
        <Button
          variant="outline"
          onClick={() => logoutMutation.mutate()}
          disabled={logoutMutation.isPending}
        >
          退出登录
        </Button>
      )}
    </div>
  )
}
