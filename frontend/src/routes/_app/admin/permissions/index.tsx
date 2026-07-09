import { useState } from 'react'
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Plus } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { FormSelect, FormText } from '@/components/form/fields'
import { UserName } from '@/components/UserName'
import { PolicyEditor } from '@/components/admin/PolicyEditor'
import { UserRolesCard } from '@/components/admin/UserRolesCard'
import { RolePicker } from '@/components/pickers/RolePicker'
import { UserPicker } from '@/components/pickers/UserPicker'
import { usePermission } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import {
  catalogOptions,
  policiesListOptions,
  rolesListOptions,
  useCreateRole,
} from '@/hooks/usePermissionsAdmin'
import { ROLE_KIND_LABELS, SUBJECT_KIND_LABELS } from '@/lib/labels'
import { notify } from '@/lib/notify'

const searchSchema = z
  .object({
    // .catch 吸收旧 ?tab=policies 书签,回落到角色 tab
    tab: z.enum(['roles', 'subjects', 'catalog']).default('roles').catch('roles'),
    subject_kind: z.enum(['user', 'role']).optional().catch(undefined),
    subject_id: z.uuid().optional().catch(undefined),
  })
  // kind 与 id 成对出现:缺/错 kind 时丢弃 id,避免把角色 UUID 当用户打开。
  .transform((search) =>
    search.subject_kind ? search : { ...search, subject_id: undefined },
  )

export const Route = createFileRoute('/_app/admin/permissions/')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'system:permissions:read'),
  staticData: { desktopOnly: true },
  component: PermissionsPage,
})

function PermissionsPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">角色与策略</h1>
      <Tabs
        value={search.tab}
        onValueChange={(value) => {
          void navigate({
            search:
              value === 'subjects'
                ? { tab: 'subjects', subject_kind: search.subject_kind, subject_id: search.subject_id }
                : { tab: value as 'roles' | 'catalog' },
          })
        }}
      >
        <TabsList>
          <TabsTrigger value="roles">角色</TabsTrigger>
          <TabsTrigger value="subjects">主体授权</TabsTrigger>
          <TabsTrigger value="catalog">权限目录</TabsTrigger>
        </TabsList>
      </Tabs>
      {search.tab === 'roles' && <RolesTab />}
      {search.tab === 'subjects' && <SubjectsTab />}
      {search.tab === 'catalog' && <CatalogTab />}
    </div>
  )
}

/* ---------------- 角色 ---------------- */

const roleFormSchema = z.object({
  code: z.string().min(1, '请填写角色编码').max(64).regex(/^[a-zA-Z0-9_:-]+$/, '仅限字母数字与 _ : -'),
  name: z.string().min(1, '请填写角色名称').max(128),
  kind: z.enum(['position', 'department', 'custom']),
})

function RolesTab() {
  const navigate = useNavigate()
  const { data: roles, isLoading } = useQuery(rolesListOptions)
  const [createOpen, setCreateOpen] = useState(false)

  return (
    <div className="flex flex-col gap-3">
      <div className="flex justify-end">
        <Button size="sm" onClick={() => setCreateOpen(true)}>
          <Plus />
          新建角色
        </Button>
      </div>
      <div className="overflow-x-auto rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>名称</TableHead>
              <TableHead>编码</TableHead>
              <TableHead>类型</TableHead>
              <TableHead className="text-right">优先级</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow>
                <TableCell colSpan={4} className="h-24 text-center text-muted-foreground">
                  加载中…
                </TableCell>
              </TableRow>
            ) : !roles || roles.length === 0 ? (
              <TableRow>
                <TableCell colSpan={4} className="h-24 text-center text-muted-foreground">
                  尚未创建角色
                </TableCell>
              </TableRow>
            ) : (
              roles.map((role) => (
                <TableRow
                  key={role.id}
                  className="cursor-pointer"
                  onClick={() => {
                    void navigate({
                      to: '/admin/permissions/roles/$roleId',
                      params: { roleId: role.id },
                    })
                  }}
                >
                  <TableCell>{role.name}</TableCell>
                  <TableCell className="font-mono text-xs">{role.code}</TableCell>
                  <TableCell>
                    <Badge variant="outline">{ROLE_KIND_LABELS[role.kind] ?? role.kind}</Badge>
                  </TableCell>
                  <TableCell className="text-right tabular-nums">{role.priority}</TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>
      {createOpen && <RoleCreateDialog onClose={() => setCreateOpen(false)} />}
    </div>
  )
}

function RoleCreateDialog({ onClose }: { onClose: () => void }) {
  const form = useForm<z.infer<typeof roleFormSchema>>({
    resolver: zodResolver(roleFormSchema),
    defaultValues: { code: '', name: '', kind: 'custom' },
  })
  const createMutation = useCreateRole()

  const submit = form.handleSubmit((values) => {
    createMutation.mutate(values, {
      onSuccess: () => {
        notify.success('角色已创建')
        onClose()
      },
      onError: (error) => notify.error(error),
    })
  })

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>新建角色</DialogTitle>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={submit}>
          <FormText control={form.control} name="name" label="角色名称" required />
          <FormText control={form.control} name="code" label="角色编码" required placeholder="如 store-manager" />
          <FormSelect
            control={form.control}
            name="kind"
            label="类型(创建后不可改)"
            required
            options={Object.entries(ROLE_KIND_LABELS).map(([value, label]) => ({ value, label }))}
          />
          <Button type="submit" disabled={createMutation.isPending}>
            {createMutation.isPending ? '创建中…' : '创建'}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}

/* ---------------- 主体授权 ---------------- */

function SubjectsTab() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const canReadUsers = usePermission('users:read')

  const kind = search.subject_kind ?? 'user'
  const subjectId = search.subject_id

  const setSubject = (nextKind: 'user' | 'role', nextId: string | undefined) => {
    void navigate({
      search: { tab: 'subjects', subject_kind: nextKind, subject_id: nextId },
    })
  }

  return (
    <div className="flex flex-col gap-4">
      <Card>
        <CardHeader>
          <CardTitle className="text-base">选择主体</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <div className="flex items-center gap-3">
            <Tabs value={kind} onValueChange={(value) => setSubject(value as 'user' | 'role', undefined)}>
              <TabsList>
                <TabsTrigger value="user">用户</TabsTrigger>
                <TabsTrigger value="role">角色</TabsTrigger>
              </TabsList>
            </Tabs>
            <div className="w-72">
              {kind === 'user' ? (
                canReadUsers ? (
                  <UserPicker value={subjectId} onChange={(id) => setSubject('user', id)} />
                ) : (
                  <p className="text-sm text-muted-foreground">
                    需要用户查看权限(users:read)才能检索人员
                  </p>
                )
              ) : (
                <RolePicker value={subjectId} onChange={(id) => setSubject('role', id)} />
              )}
            </div>
          </div>
        </CardContent>
      </Card>

      {subjectId ? (
        kind === 'user' ? (
          // key 绑定主体:切换主体时强制重挂,防止上一主体的未保存草稿渗入。
          <>
            <UserRolesCard key={subjectId} userId={subjectId} />
            <Card>
              <CardHeader>
                <CardTitle className="text-base">个人策略</CardTitle>
              </CardHeader>
              <CardContent>
                <PolicyEditor key={`user:${subjectId}`} subjectKind="user" subjectId={subjectId} />
              </CardContent>
            </Card>
          </>
        ) : (
          <Card>
            <CardHeader>
              <CardTitle className="text-base">角色策略</CardTitle>
              <p className="text-sm text-muted-foreground">
                基本信息、父角色与成员在
                <Link
                  to="/admin/permissions/roles/$roleId"
                  params={{ roleId: subjectId }}
                  className="mx-1 underline underline-offset-2"
                >
                  角色详情
                </Link>
                中管理。
              </p>
            </CardHeader>
            <CardContent>
              <PolicyEditor key={`role:${subjectId}`} subjectKind="role" subjectId={subjectId} />
            </CardContent>
          </Card>
        )
      ) : (
        <ConfiguredSubjects onSelect={setSubject} />
      )}
    </div>
  )
}

/** 主体发现:按 (subject_kind, subject_id) 分组的全部策略总览,点击进入编辑。 */
function ConfiguredSubjects({
  onSelect,
}: {
  onSelect: (kind: 'user' | 'role', id: string) => void
}) {
  const { data: policies, isLoading, isError } = useQuery(policiesListOptions())
  const { data: roles } = useQuery(rolesListOptions)

  const groups = new Map<string, { kind: 'user' | 'role'; id: string; count: number }>()
  for (const policy of policies ?? []) {
    const key = `${policy.subject_kind}:${policy.subject_id}`
    const entry = groups.get(key)
    if (entry) {
      entry.count += 1
    } else {
      groups.set(key, { kind: policy.subject_kind, id: policy.subject_id, count: 1 })
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">已配置策略的主体</CardTitle>
        <p className="text-sm text-muted-foreground">
          选择上方主体开始编辑,或从下列已有配置直接进入。
        </p>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <p className="text-sm text-muted-foreground">加载中…</p>
        ) : isError ? (
          <p className="text-sm text-destructive">策略列表加载失败,请刷新重试</p>
        ) : groups.size === 0 ? (
          <p className="text-sm text-muted-foreground">暂无策略</p>
        ) : (
          <div className="flex flex-col">
            {[...groups.values()].map((subject) => (
              <button
                key={`${subject.kind}:${subject.id}`}
                type="button"
                className="flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-muted"
                onClick={() => onSelect(subject.kind, subject.id)}
              >
                <Badge variant="outline">{SUBJECT_KIND_LABELS[subject.kind]}</Badge>
                <span className="flex-1 truncate">
                  {subject.kind === 'role' ? (
                    (roles?.find((r) => r.id === subject.id)?.name ?? subject.id.slice(0, 8))
                  ) : (
                    <UserName userId={subject.id} />
                  )}
                </span>
                <Badge variant="secondary">{subject.count} 条策略</Badge>
              </button>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

/* ---------------- 权限目录 ---------------- */

function CatalogTab() {
  const { data: catalog, isLoading } = useQuery(catalogOptions)
  if (isLoading) return <p className="text-sm text-muted-foreground">加载中…</p>
  const groups = new Map<string, NonNullable<typeof catalog>>()
  for (const entry of catalog ?? []) {
    const list = groups.get(entry.group) ?? []
    list.push(entry)
    groups.set(entry.group, list)
  }

  return (
    <div className="grid gap-3 md:grid-cols-2">
      {[...groups.entries()].map(([group, entries]) => (
        <Card key={group}>
          <CardHeader>
            <CardTitle className="text-base">{group}</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-2">
            {entries.map((entry) => (
              <div key={entry.object} className="flex items-center justify-between gap-2 text-sm">
                <div>
                  <p>{entry.label}</p>
                  <p className="font-mono text-xs text-muted-foreground">{entry.object}</p>
                </div>
                <div className="flex gap-1">
                  {entry.actions.map((action) => (
                    <Badge key={action} variant="secondary">
                      {action}
                    </Badge>
                  ))}
                </div>
              </div>
            ))}
          </CardContent>
        </Card>
      ))}
    </div>
  )
}
