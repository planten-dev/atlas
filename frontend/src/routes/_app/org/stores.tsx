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
import { SystemPicker } from '@/components/pickers/SystemPicker'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import {
  storesListOptions,
  useCreateStore,
  useDisableStore,
  useUpdateStore,
  type StoreResponse,
} from '@/hooks/useStores'
import { notify } from '@/lib/notify'
import { outcomeMessage } from '@/hooks/mutation-result'

const searchSchema = z.object({
  system_id: z.string().optional(),
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

export const Route = createFileRoute('/_app/org/stores')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'stores:read'),
  staticData: { desktopOnly: true },
  component: StoresPage,
})

function StoresPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(storesListOptions(search))
  const [editing, setEditing] = useState<StoreResponse | 'new' | null>(null)
  const disableMutation = useDisableStore()
  const rows = query.data?.items ?? []

  const columns: ColumnDef<StoreResponse>[] = [
    { accessorKey: 'name', header: '门店名称' },
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
        <Guard perm="stores:write">
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
      <h1 className="text-xl font-semibold">门店管理</h1>
      <DataTableToolbar
        actions={
          <Guard perm="stores:write">
            <Button size="sm" onClick={() => setEditing('new')}>
              <Plus />
              新建门店
            </Button>
          </Guard>
        }
      >
        <SystemPicker
          value={search.system_id}
          onChange={(v) => {
            void navigate({ search: (prev) => ({ ...prev, system_id: v, page_number: 1 }) })
          }}
          placeholder="按体系筛选"
          className="w-56"
        />
      </DataTableToolbar>
      <DataTable
        tableId="org-stores"
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
        <StoreDialog store={editing === 'new' ? null : editing} onClose={() => setEditing(null)} />
      )}
    </div>
  )
}

const storeFormSchema = z.object({
  name: z.string().min(1, '请填写门店名称').max(128),
  system_id: z.string().min(1, '请选择体系'),
})

function StoreDialog({ store, onClose }: { store: StoreResponse | null; onClose: () => void }) {
  const form = useForm<z.infer<typeof storeFormSchema>>({
    resolver: zodResolver(storeFormSchema),
    defaultValues: {
      name: store?.name ?? '',
      system_id: store?.system_id ?? '',
    },
  })
  const createMutation = useCreateStore()
  const updateMutation = useUpdateStore()
  const isPending = createMutation.isPending || updateMutation.isPending

  const submit = form.handleSubmit((values) => {
    const callbacks = {
      onSuccess: (outcome: Parameters<typeof outcomeMessage>[0]) => {
        notify.success(outcomeMessage(outcome, store ? '门店已更新' : '门店已创建'))
        onClose()
      },
      onError: (error: unknown) => notify.error(error),
    }
    if (store) {
      updateMutation.mutate(
        { storeId: store.id, body: { name: values.name, system_id: values.system_id } },
        callbacks,
      )
    } else {
      createMutation.mutate(
        { name: values.name, system_id: values.system_id, status: 'active' },
        callbacks,
      )
    }
  })

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{store ? '编辑门店' : '新建门店'}</DialogTitle>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={submit}>
          <FormText control={form.control} name="name" label="门店名称" required />
          <div className="flex flex-col gap-1.5">
            <span className="text-sm font-medium">
              体系<span className="text-destructive">*</span>
            </span>
            <SystemPicker
              value={form.watch('system_id') || undefined}
              onChange={(v) => form.setValue('system_id', v ?? '', { shouldValidate: true })}
            />
            {form.formState.errors.system_id && (
              <p className="text-sm text-destructive">{form.formState.errors.system_id.message}</p>
            )}
          </div>
          <Button type="submit" disabled={isPending}>
            {isPending ? '保存中…' : '保存'}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}
