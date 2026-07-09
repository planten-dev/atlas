import { useState } from 'react'
import { createFileRoute, Link } from '@tanstack/react-router'
import { useQuery, useSuspenseQuery } from '@tanstack/react-query'
import { ArrowLeft, RefreshCw } from 'lucide-react'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { PolicyEditor } from '@/components/admin/PolicyEditor'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import { userProfileQueryOptions } from '@/hooks/useUserProfile'
import { useSyncUserProfile } from '@/hooks/useUsers'
import {
  rolesListOptions,
  userRolesOptions,
  useSetUserRoles,
} from '@/hooks/usePermissionsAdmin'
import { ROLE_KIND_LABELS } from '@/lib/labels'
import { notify } from '@/lib/notify'

export const Route = createFileRoute('/_app/admin/users/$userId')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'users:read'),
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(userProfileQueryOptions(params.userId)),
  staticData: { desktopOnly: true },
  component: UserDetailPage,
})

function UserDetailPage() {
  const { userId } = Route.useParams()
  const { data: profile } = useSuspenseQuery(userProfileQueryOptions(userId))
  const syncMutation = useSyncUserProfile()

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="icon-sm"
          render={<Link to="/admin/users" search={{}} aria-label="返回" />}
        >
          <ArrowLeft />
        </Button>
        <h1 className="flex-1 text-xl font-semibold">用户详情</h1>
        <Guard perm="users:write">
          <Button
            variant="outline"
            size="sm"
            disabled={syncMutation.isPending}
            onClick={() => {
              syncMutation.mutate(userId, {
                onSuccess: () => notify.success('已从钉钉同步资料'),
                onError: (error) => notify.error(error),
              })
            }}
          >
            <RefreshCw />
            从钉钉同步
          </Button>
        </Guard>
      </div>

      <Card>
        <CardContent className="flex items-center gap-4 py-4">
          <Avatar className="size-14">
            <AvatarImage src={profile.avatar_url ?? undefined} />
            <AvatarFallback>{profile.name?.slice(0, 1) ?? '?'}</AvatarFallback>
          </Avatar>
          <div className="flex flex-col gap-0.5 text-sm">
            <p className="text-base font-medium">{profile.name ?? '未同步'}</p>
            <p className="text-muted-foreground">
              {[profile.title, profile.job_number, profile.mobile].filter(Boolean).join(' · ') || '-'}
            </p>
            <p className="text-muted-foreground">{profile.email ?? profile.org_email ?? ''}</p>
          </div>
          <div className="ml-auto">
            {profile.is_active === false ? (
              <Badge variant="destructive">离职/停用</Badge>
            ) : (
              <Badge variant="outline">在职</Badge>
            )}
          </div>
        </CardContent>
      </Card>

      <Guard perm="system:permissions:read">
        <RolesCard userId={userId} />
      </Guard>

      <Guard perm="system:permissions:read">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">个人策略</CardTitle>
          </CardHeader>
          <CardContent>
            <PolicyEditor subjectKind="user" subjectId={userId} />
          </CardContent>
        </Card>
      </Guard>
    </div>
  )
}

function RolesCard({ userId }: { userId: string }) {
  const { data: allRoles } = useQuery(rolesListOptions)
  const { data: userRoles } = useQuery(userRolesOptions(userId))
  const setRolesMutation = useSetUserRoles()
  const [draft, setDraft] = useState<Set<string> | null>(null)

  const assigned = draft ?? new Set((userRoles?.roles ?? []).map((r) => r.id))
  const dirty = draft !== null

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle className="text-base">角色分配</CardTitle>
        {dirty && (
          <div className="flex gap-2">
            <Button
              size="sm"
              disabled={setRolesMutation.isPending}
              onClick={() => {
                setRolesMutation.mutate(
                  { userId, roleIds: [...assigned] },
                  {
                    onSuccess: () => {
                      notify.success('角色已更新')
                      setDraft(null)
                    },
                    onError: (error) => notify.error(error),
                  },
                )
              }}
            >
              保存
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setDraft(null)}>
              放弃
            </Button>
          </div>
        )}
      </CardHeader>
      <CardContent>
        {!allRoles ? (
          <p className="text-sm text-muted-foreground">加载中…</p>
        ) : allRoles.length === 0 ? (
          <p className="text-sm text-muted-foreground">尚未创建任何角色</p>
        ) : (
          <div className="flex flex-col gap-2">
            {allRoles.map((role) => (
              <label key={role.id} className="flex cursor-pointer items-center gap-2 text-sm">
                <Checkbox
                  checked={assigned.has(role.id)}
                  onCheckedChange={(checked) => {
                    const next = new Set(assigned)
                    if (checked) {
                      next.add(role.id)
                    } else {
                      next.delete(role.id)
                    }
                    setDraft(next)
                  }}
                />
                <span>{role.name}</span>
                <Badge variant="outline">{ROLE_KIND_LABELS[role.kind] ?? role.kind}</Badge>
                <span className="text-xs text-muted-foreground">{role.code}</span>
              </label>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
