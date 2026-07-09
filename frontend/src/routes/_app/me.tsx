import type { ReactNode } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useMutation, useQueryClient, useSuspenseQuery } from '@tanstack/react-query'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Separator } from '@/components/ui/separator'
import { logout, meQueryOptions } from '@/auth/session'
import { usePermissions } from '@/auth/PermissionProvider'
import { useIsMobile } from '@/hooks/use-mobile'
import { useUserProfile } from '@/hooks/useUserProfile'
import { formatDate, formatDateTime } from '@/lib/date'
import { groupPermissions, PERMISSION_ACTION_LABELS } from '@/lib/labels'
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

  const email = profile?.email ?? profile?.org_email
  const mobile = profile?.mobile ?? (profile?.hide_mobile ? '已设为隐藏' : null)

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4">
      <h1 className="text-xl font-semibold">个人中心</h1>

      {/* 身份:是谁、什么身份 */}
      <Card>
        <CardContent className="flex items-center gap-4 py-4">
          <Avatar className="size-14">
            <AvatarImage src={profile?.avatar_url ?? undefined} />
            <AvatarFallback>{profile?.name?.slice(0, 1) ?? '?'}</AvatarFallback>
          </Avatar>
          <div className="flex min-w-0 flex-col gap-0.5">
            <div className="flex flex-wrap items-center gap-2">
              <p className="font-medium">{profile?.name ?? '未同步'}</p>
              {profile?.is_admin && <Badge variant="secondary">管理员</Badge>}
              {profile?.is_boss && <Badge variant="secondary">老板</Badge>}
              {profile?.is_senior && <Badge variant="secondary">高管</Badge>}
              {(me.status === 'disabled' || profile?.is_active === false) && (
                <Badge variant="destructive">停用</Badge>
              )}
            </div>
            <p className="text-sm text-muted-foreground">
              {[profile?.title, profile?.job_number && `工号 ${profile.job_number}`]
                .filter(Boolean)
                .join(' · ') || '暂无职位信息'}
            </p>
          </div>
        </CardContent>
      </Card>

      {/* 资料:联系方式与工作信息(来自钉钉同步) */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">工作资料</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col">
          <InfoRow label="手机" value={mobile} />
          <InfoRow label="座机" value={profile?.telephone} />
          <InfoRow label="邮箱" value={email} />
          <InfoRow label="办公地点" value={profile?.work_place} />
          <InfoRow label="入职时间" value={profile?.hired_at && formatDate(profile.hired_at)} />
          <InfoRow label="备注" value={profile?.remark} last />
        </CardContent>
      </Card>

      {/* 账号:登录与同步状态 */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">账号信息</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col">
          <InfoRow label="上次登录" value={formatDateTime(me.last_login_at)} />
          <InfoRow label="账号创建" value={formatDateTime(me.created_at)} />
          <InfoRow
            label="资料同步于"
            value={profile ? formatDateTime(profile.updated_at) : null}
            last
          />
        </CardContent>
      </Card>

      {/* 权限:按目录分组,展示中文对象名 + 动作 */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">我的权限</CardTitle>
        </CardHeader>
        <CardContent>
          {permissions.size === 0 ? (
            <p className="text-sm text-muted-foreground">暂无权限,请联系管理员</p>
          ) : (
            <div className="flex flex-col gap-4">
              {groupPermissions(permissions).map((group) => (
                <div key={group.group} className="flex flex-col gap-2">
                  <p className="text-xs font-medium text-muted-foreground">{group.group}</p>
                  <div className="flex flex-col gap-1.5">
                    {group.items.map((item) => (
                      <div key={item.object} className="flex flex-wrap items-center gap-1.5 text-sm">
                        <span className="min-w-24">{item.label}</span>
                        {item.actions.map((action) => (
                          <Badge key={action} variant="secondary">
                            {PERMISSION_ACTION_LABELS[action] ?? action}
                          </Badge>
                        ))}
                      </div>
                    ))}
                  </div>
                </div>
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

/** 键值行:空值统一显示 '-',保持行位稳定可扫读。 */
function InfoRow({
  label,
  value,
  last,
}: {
  label: string
  value: ReactNode | null | undefined
  last?: boolean
}) {
  return (
    <>
      <div className="flex items-center justify-between gap-4 py-2 text-sm">
        <span className="shrink-0 text-muted-foreground">{label}</span>
        <span className="min-w-0 truncate text-right">{value || '-'}</span>
      </div>
      {!last && <Separator />}
    </>
  )
}
