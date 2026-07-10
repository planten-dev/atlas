import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import type { ColumnDef } from '@tanstack/react-table'
import { Plus } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { DataTable } from '@/components/data-table/data-table'
import { DataTableToolbar } from '@/components/data-table/toolbar'
import { FormText } from '@/components/form/fields'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import {
  systemsListOptions,
  useCreateSystem,
  useDisableSystem,
  useUpdateSystem,
  type SystemResponse,
} from '@/hooks/useSystems'
import { notify } from '@/lib/notify'
import { outcomeMessage } from '@/hooks/mutation-result'

const searchSchema = z.object({
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

export const Route = createFileRoute('/_app/org/systems')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'systems:read'),
  staticData: { tab: 'admin' },
  component: SystemsPage,
})

const systemFormSchema = z.object({
  name: z.string().min(1, '请填写体系名称').max(128),
})

function SystemsPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(systemsListOptions(search))
  const [editing, setEditing] = useState<SystemResponse | 'new' | null>(null)
  const disableMutation = useDisableSystem()
  const rows = query.data?.items ?? []

  const columns: ColumnDef<SystemResponse>[] = [
    { accessorKey: 'name', header: '体系名称' },
    {
      accessorKey: 'status',
      header: '状态',
      cell: ({ row }) =>
        row.original.status === 'disabled' ? (
          <Badge variant="destructive">停用</Badge>
        ) : (
          <Badge variant="outline">启用</Badge>
        ),
    },
    {
      id: 'actions',
      header: '',
      cell: ({ row }) => (
        <Guard perm="systems:write">
          <div className="flex justify-end gap-1">
            <Button variant="ghost" size="xs" onClick={() => setEditing(row.original)}>
              编辑
            </Button>
            {row.original.status === 'active' && (
              <Button
                variant="ghost"
                size="xs"
                onClick={() => {
                  disableMutation.mutate(row.original.id, {
                    onSuccess: (outcome) => notify.success(outcomeMessage(outcome, '已停用')),
                    onError: (error) => notify.error(error),
                  })
                }}
              >
                停用
              </Button>
            )}
          </div>
        </Guard>
      ),
    },
  ]

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">体系管理</h1>
      <DataTableToolbar
        actions={
          <Guard perm="systems:write">
            <Button onClick={() => setEditing('new')}>
              <Plus />
              新建体系
            </Button>
          </Guard>
        }
      />
      <DataTable
        tableId="org-systems"
        columns={columns}
        data={rows}
        totalCount={query.data?.totalCount ?? 0}
        isLoading={query.isLoading}
        page={{ pageNumber: search.page_number, pageSize: search.page_size }}
        onPageChange={(page) => {
          void navigate({
            search: (prev) => ({ ...prev, page_number: page.pageNumber, page_size: page.pageSize }),
          })
        }}
      />
      {editing && (
        <SystemDialog system={editing === 'new' ? null : editing} onClose={() => setEditing(null)} />
      )}
    </div>
  )
}

function SystemDialog({ system, onClose }: { system: SystemResponse | null; onClose: () => void }) {
  const form = useForm<z.infer<typeof systemFormSchema>>({
    resolver: zodResolver(systemFormSchema),
    defaultValues: {
      name: system?.name ?? '',
    },
  })
  const createMutation = useCreateSystem()
  const updateMutation = useUpdateSystem()
  const isPending = createMutation.isPending || updateMutation.isPending

  const submit = form.handleSubmit((values) => {
    const callbacks = {
      onSuccess: (outcome: Parameters<typeof outcomeMessage>[0]) => {
        notify.success(outcomeMessage(outcome, system ? '体系已更新' : '体系已创建'))
        onClose()
      },
      onError: (error: unknown) => notify.error(error),
    }
    if (system) {
      updateMutation.mutate({ systemId: system.id, body: values }, callbacks)
    } else {
      createMutation.mutate({ ...values, status: 'active' }, callbacks)
    }
  })

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{system ? '编辑体系' : '新建体系'}</DialogTitle>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={submit}>
          <FormText control={form.control} name="name" label="体系名称" required />
          <Button type="submit" disabled={isPending}>
            {isPending ? '保存中…' : '保存'}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}
