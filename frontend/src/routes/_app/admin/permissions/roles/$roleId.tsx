import { useState } from 'react'
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useQuery, useSuspenseQuery } from '@tanstack/react-query'
import { ArrowLeft, Plus, ShieldCheck, Trash2 } from 'lucide-react'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardAction, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { UserName } from '@/components/UserName'
import { UserPicker } from '@/components/pickers/UserPicker'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import {
  roleDetailOptions,
  rolesListOptions,
  roleUsersOptions,
  useDeleteRole,
  useSetRoleParents,
  useToggleRoleMember,
  useUpdateRole,
} from '@/hooks/usePermissionsAdmin'
import { ROLE_KIND_LABELS } from '@/lib/labels'
import { notify } from '@/lib/notify'

export const Route = createFileRoute('/_app/admin/permissions/roles/$roleId')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'system:permissions:read'),
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(roleDetailOptions(params.roleId)),
  staticData: { desktopOnly: true },
  component: RoleDetailPage,
})

function RoleDetailPage() {
  const { roleId } = Route.useParams()
  const navigate = useNavigate()
  const { data: role } = useSuspenseQuery(roleDetailOptions(roleId))
  const deleteMutation = useDeleteRole()

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="icon-sm"
          render={<Link to="/admin/permissions" search={{ tab: 'roles' }} aria-label="返回" />}
        >
          <ArrowLeft />
        </Button>
        <h1 className="flex-1 text-xl font-semibold">
          {role.name}
          <Badge variant="outline" className="ml-2 align-middle">
            {ROLE_KIND_LABELS[role.kind] ?? role.kind}
          </Badge>
        </h1>
        <AlertDialog>
          <AlertDialogTrigger render={<Button variant="destructive" size="sm" />}>
            <Trash2 />
            删除角色
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>删除角色 “{role.name}”?</AlertDialogTitle>
              <AlertDialogDescription>
                删除后,该角色的所有分配与策略将失效。
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>取消</AlertDialogCancel>
              <AlertDialogAction
                onClick={() => {
                  deleteMutation.mutate(roleId, {
                    onSuccess: () => {
                      notify.success('角色已删除')
                      void navigate({ to: '/admin/permissions', search: { tab: 'roles' } })
                    },
                    onError: (error) => notify.error(error),
                  })
                }}
              >
                确认删除
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </div>

      <BasicCard roleId={roleId} name={role.name} code={role.code} priority={role.priority} />
      <ParentsCard roleId={roleId} parentRoleIds={role.parent_role_ids} />
      <MembersCard roleId={roleId} />

      <Card>
        <CardHeader>
          <CardTitle className="text-base">角色策略</CardTitle>
        </CardHeader>
        <CardContent>
          <Button
            variant="outline"
            size="sm"
            render={
              <Link
                to="/admin/permissions"
                search={{ tab: 'subjects', subject_kind: 'role', subject_id: roleId }}
              />
            }
          >
            <ShieldCheck />
            在主体授权中编辑角色策略
          </Button>
        </CardContent>
      </Card>
    </div>
  )
}

function MembersCard({ roleId }: { roleId: string }) {
  const { data: roleUsers, isLoading, isError } = useQuery(roleUsersOptions(roleId))
  const toggleMutation = useToggleRoleMember()
  const [pendingUserId, setPendingUserId] = useState<string | undefined>(undefined)
  const members = roleUsers?.users ?? []
  const memberIds = members.map((u) => u.id)

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">成员</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {isLoading ? (
          <p className="text-sm text-muted-foreground">加载中…</p>
        ) : isError ? (
          <p className="text-sm text-destructive">成员列表加载失败,请刷新重试</p>
        ) : members.length === 0 ? (
          <p className="text-sm text-muted-foreground">该角色暂无成员</p>
        ) : (
          <div className="flex flex-col">
            {members.map((member) => (
              <div
                key={member.id}
                className="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-muted"
              >
                <span className="flex-1 truncate">
                  <UserName userId={member.id} />
                </span>
                {member.status === 'disabled' && <Badge variant="destructive">停用</Badge>}
                <Guard perm="system:permissions:write">
                  <AlertDialog>
                    <AlertDialogTrigger
                      render={<Button variant="ghost" size="icon-xs" aria-label="移出成员" />}
                    >
                      <Trash2 />
                    </AlertDialogTrigger>
                    <AlertDialogContent>
                      <AlertDialogHeader>
                        <AlertDialogTitle>
                          将 “<UserName userId={member.id} />” 移出该角色?
                        </AlertDialogTitle>
                        <AlertDialogDescription>
                          移出后该用户将立即失去此角色带来的全部权限。
                        </AlertDialogDescription>
                      </AlertDialogHeader>
                      <AlertDialogFooter>
                        <AlertDialogCancel>取消</AlertDialogCancel>
                        <AlertDialogAction
                          onClick={() => {
                            toggleMutation.mutate(
                              { roleId, userId: member.id, op: 'remove' },
                              {
                                onSuccess: () => notify.success('已移出成员'),
                                onError: (error) => notify.error(error),
                              },
                            )
                          }}
                        >
                          确认移出
                        </AlertDialogAction>
                      </AlertDialogFooter>
                    </AlertDialogContent>
                  </AlertDialog>
                </Guard>
              </div>
            ))}
          </div>
        )}
        <Guard perm="system:permissions:write">
          <Guard perm="users:read">
            <div className="flex items-center gap-2">
              <div className="w-64">
                <UserPicker
                  value={pendingUserId}
                  onChange={setPendingUserId}
                  disabledIds={memberIds}
                  placeholder="选择要添加的人员"
                />
              </div>
              <Button
                size="sm"
                disabled={!pendingUserId || toggleMutation.isPending}
                onClick={() => {
                  if (!pendingUserId) return
                  toggleMutation.mutate(
                    { roleId, userId: pendingUserId, op: 'add' },
                    {
                      onSuccess: () => {
                        notify.success('已添加成员')
                        setPendingUserId(undefined)
                      },
                      onError: (error) => notify.error(error),
                    },
                  )
                }}
              >
                <Plus />
                添加成员
              </Button>
            </div>
          </Guard>
        </Guard>
      </CardContent>
    </Card>
  )
}

function BasicCard({
  roleId,
  name,
  code,
  priority,
}: {
  roleId: string
  name: string
  code: string
  priority: number
}) {
  const [draftName, setDraftName] = useState(name)
  const [draftPriority, setDraftPriority] = useState(String(priority))
  const updateMutation = useUpdateRole()
  const dirty = draftName !== name || draftPriority !== String(priority)

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">基本信息</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="grid gap-3 md:grid-cols-3">
          <label className="flex flex-col gap-1.5 text-sm">
            名称
            <Input value={draftName} onChange={(e) => setDraftName(e.target.value)} />
          </label>
          <label className="flex flex-col gap-1.5 text-sm">
            编码(不可改)
            <Input value={code} disabled />
          </label>
          <label className="flex flex-col gap-1.5 text-sm">
            优先级(小者优先)
            <Input
              type="number"
              value={draftPriority}
              onChange={(e) => setDraftPriority(e.target.value)}
            />
          </label>
        </div>
        {dirty && (
          <div className="flex gap-2">
            <Button
              size="sm"
              disabled={updateMutation.isPending}
              onClick={() => {
                const p = Number(draftPriority)
                updateMutation.mutate(
                  {
                    roleId,
                    body: {
                      name: draftName || null,
                      priority: Number.isInteger(p) ? p : null,
                    },
                  },
                  {
                    onSuccess: () => notify.success('角色已更新'),
                    onError: (error) => notify.error(error),
                  },
                )
              }}
            >
              保存
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                setDraftName(name)
                setDraftPriority(String(priority))
              }}
            >
              放弃
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function ParentsCard({ roleId, parentRoleIds }: { roleId: string; parentRoleIds: string[] }) {
  const { data: allRoles } = useQuery(rolesListOptions)
  const setParentsMutation = useSetRoleParents()
  const [draft, setDraft] = useState<Set<string> | null>(null)
  const selected = draft ?? new Set(parentRoleIds)
  const dirty = draft !== null
  const candidates = (allRoles ?? []).filter((r) => r.id !== roleId)

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">父角色(继承其权限)</CardTitle>
        {dirty && (
          <CardAction className="flex gap-2">
            <Button
              size="sm"
              disabled={setParentsMutation.isPending}
              onClick={() => {
                setParentsMutation.mutate(
                  { roleId, parentRoleIds: [...selected] },
                  {
                    onSuccess: () => {
                      notify.success('父角色已更新')
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
        {candidates.length === 0 ? (
          <p className="text-sm text-muted-foreground">没有其他角色可选</p>
        ) : (
          <div className="flex flex-col gap-2">
            {candidates.map((candidate) => (
              <label key={candidate.id} className="flex cursor-pointer items-center gap-2 text-sm">
                <Checkbox
                  checked={selected.has(candidate.id)}
                  onCheckedChange={(checked) => {
                    const next = new Set(selected)
                    if (checked) {
                      next.add(candidate.id)
                    } else {
                      next.delete(candidate.id)
                    }
                    setDraft(next)
                  }}
                />
                <span>{candidate.name}</span>
                <span className="font-mono text-xs text-muted-foreground">{candidate.code}</span>
              </label>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
