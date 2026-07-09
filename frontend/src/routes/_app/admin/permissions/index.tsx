import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Plus, Trash2 } from 'lucide-react'
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
import { requirePerm } from '@/auth/route-guard'
import {
  catalogOptions,
  policiesListOptions,
  rolesListOptions,
  useCreateRole,
  useDeletePolicy,
  type PolicyResponse,
} from '@/hooks/usePermissionsAdmin'
import { POLICY_EFFECT_LABELS, ROLE_KIND_LABELS, SUBJECT_KIND_LABELS } from '@/lib/labels'
import { notify } from '@/lib/notify'

const searchSchema = z.object({
  tab: z.enum(['roles', 'policies', 'catalog']).default('roles'),
})

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
          void navigate({ search: { tab: value as 'roles' | 'policies' | 'catalog' } })
        }}
      >
        <TabsList>
          <TabsTrigger value="roles">角色</TabsTrigger>
          <TabsTrigger value="policies">策略</TabsTrigger>
          <TabsTrigger value="catalog">权限目录</TabsTrigger>
        </TabsList>
      </Tabs>
      {search.tab === 'roles' && <RolesTab />}
      {search.tab === 'policies' && <PoliciesTab />}
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

/* ---------------- 策略 ---------------- */

function PoliciesTab() {
  const { data: policies, isLoading } = useQuery(policiesListOptions())
  const deleteMutation = useDeletePolicy()

  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-muted-foreground">
        全部策略总览。新增请在用户详情/角色详情的策略编辑器中操作。
      </p>
      <div className="overflow-x-auto rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>主体</TableHead>
              <TableHead>资源</TableHead>
              <TableHead>操作</TableHead>
              <TableHead>效果</TableHead>
              <TableHead />
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow>
                <TableCell colSpan={5} className="h-24 text-center text-muted-foreground">
                  加载中…
                </TableCell>
              </TableRow>
            ) : !policies || policies.length === 0 ? (
              <TableRow>
                <TableCell colSpan={5} className="h-24 text-center text-muted-foreground">
                  暂无策略
                </TableCell>
              </TableRow>
            ) : (
              policies.map((policy) => (
                <PolicyRow
                  key={policy.id}
                  policy={policy}
                  onDelete={() => {
                    deleteMutation.mutate(policy.id, {
                      onSuccess: () => notify.success('策略已删除'),
                      onError: (error) => notify.error(error),
                    })
                  }}
                />
              ))
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  )
}

function PolicyRow({ policy, onDelete }: { policy: PolicyResponse; onDelete: () => void }) {
  const { data: roles } = useQuery(rolesListOptions)
  const subjectLabel =
    policy.subject_kind === 'role' ? (
      (roles?.find((r) => r.id === policy.subject_id)?.name ?? policy.subject_id.slice(0, 8))
    ) : (
      <UserName userId={policy.subject_id} />
    )

  return (
    <TableRow>
      <TableCell>
        <Badge variant="outline" className="mr-1">
          {SUBJECT_KIND_LABELS[policy.subject_kind]}
        </Badge>
        {subjectLabel}
      </TableCell>
      <TableCell className="font-mono text-xs">{policy.object}</TableCell>
      <TableCell className="font-mono text-xs">{policy.action}</TableCell>
      <TableCell>
        <Badge variant={policy.effect === 'deny' ? 'destructive' : 'outline'}>
          {POLICY_EFFECT_LABELS[policy.effect]}
        </Badge>
      </TableCell>
      <TableCell className="text-right">
        <AlertDialog>
          <AlertDialogTrigger
            render={<Button variant="ghost" size="icon-xs" aria-label="删除策略" />}
          >
            <Trash2 />
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>删除该策略?</AlertDialogTitle>
              <AlertDialogDescription>
                {policy.object} / {policy.action} / {POLICY_EFFECT_LABELS[policy.effect]}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>取消</AlertDialogCancel>
              <AlertDialogAction onClick={onDelete}>删除</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </TableCell>
    </TableRow>
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
