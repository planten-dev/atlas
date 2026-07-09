import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardAction, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { usePermission } from '@/auth/PermissionProvider'
import { rolesListOptions, userRolesOptions, useSetUserRoles } from '@/hooks/usePermissionsAdmin'
import { ROLE_KIND_LABELS } from '@/lib/labels'
import { notify } from '@/lib/notify'

/** 用户角色分配编辑器:全量角色勾选 + 整集替换保存(权限面板·主体授权用)。 */
export function UserRolesCard({ userId }: { userId: string }) {
  const { data: allRoles, isError: rolesError } = useQuery(rolesListOptions)
  const userRolesQuery = useQuery(userRolesOptions(userId))
  const canWrite = usePermission('system:permissions:write')
  const setRolesMutation = useSetUserRoles()
  const [draft, setDraft] = useState<Set<string> | null>(null)

  // 当前角色集未加载完成前禁止编辑:否则草稿会从空集起步,
  // 保存时把用户已有角色整集清掉。
  const loaded = userRolesQuery.data !== undefined
  const assigned = draft ?? new Set((userRolesQuery.data?.roles ?? []).map((r) => r.id))
  const dirty = draft !== null

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">角色分配</CardTitle>
        {dirty && (
          <CardAction className="flex gap-2">
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
          </CardAction>
        )}
      </CardHeader>
      <CardContent>
        {rolesError || userRolesQuery.isError ? (
          <p className="text-sm text-destructive">角色数据加载失败,请刷新重试</p>
        ) : !allRoles || !loaded ? (
          <p className="text-sm text-muted-foreground">加载中…</p>
        ) : allRoles.length === 0 ? (
          <p className="text-sm text-muted-foreground">尚未创建任何角色</p>
        ) : (
          <div className="flex flex-col gap-2">
            {allRoles.map((role) => (
              <label
                key={role.id}
                className={`flex items-center gap-2 text-sm ${canWrite ? 'cursor-pointer' : ''}`}
              >
                <Checkbox
                  checked={assigned.has(role.id)}
                  disabled={!canWrite}
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
