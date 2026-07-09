import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import type { ColumnDef } from '@tanstack/react-table'
import { Plus } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { DataTable } from '@/components/data-table/data-table'
import { DataTableToolbar } from '@/components/data-table/toolbar'
import { SystemPicker } from '@/components/pickers/SystemPicker'
import { StorePicker } from '@/components/pickers/StorePicker'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import { customersListOptions, type CustomerResponse } from '@/hooks/useCustomers'
import { formatDate } from '@/lib/date'
import { ENTITY_STATUS_LABELS } from '@/lib/labels'

const searchSchema = z.object({
  name_keyword: z.string().optional(),
  system_id: z.string().optional(),
  store_id: z.string().optional(),
  status_filter: z.enum(['active', 'disabled']).optional(),
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

type CustomersSearch = z.infer<typeof searchSchema>

export const Route = createFileRoute('/_app/customers/')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'customers:read'),
  staticData: { tab: 'home' },
  component: CustomersListPage,
})

const columns: ColumnDef<CustomerResponse>[] = [
  { accessorKey: 'name', header: '姓名' },
  {
    accessorKey: 'remark',
    header: '备注',
    cell: ({ row }) => (
      <span className="line-clamp-1 text-muted-foreground">{row.original.remark ?? '-'}</span>
    ),
  },
  {
    accessorKey: 'created_at',
    header: '创建时间',
    cell: ({ row }) => formatDate(row.original.created_at),
  },
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
]

function CustomersListPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(customersListOptions(search))
  const rows = query.data?.items ?? []

  const patchSearch = (patch: Partial<CustomersSearch>) => {
    void navigate({ search: (prev) => ({ ...prev, ...patch, page_number: 1 }) })
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">客户</h1>
        <Guard perm="customers:write">
          <Button render={<Link to="/customers/new" />}>
            <Plus />
            新建客户
          </Button>
        </Guard>
      </div>

      <DataTableToolbar
        exportConfig={{
          filename: `客户-${new Date().toISOString().slice(0, 10)}`,
          columns: [
            { header: '姓名', value: (r: CustomerResponse) => r.name },
            { header: '备注', value: (r) => r.remark },
            { header: '状态', value: (r) => ENTITY_STATUS_LABELS[r.status] ?? r.status },
            { header: '创建时间', value: (r) => formatDate(r.created_at) },
          ],
          rows,
        }}
      >
        <div className="grid w-full gap-2 md:grid-cols-4">
          <Input
            placeholder="搜索客户姓名…"
            defaultValue={search.name_keyword ?? ''}
            onChange={(e) => patchSearch({ name_keyword: e.target.value || undefined })}
          />
          <SystemPicker
            value={search.system_id}
            onChange={(v) => patchSearch({ system_id: v, store_id: undefined })}
            placeholder="按体系筛选"
          />
          <StorePicker
            systemId={search.system_id}
            value={search.store_id}
            onChange={(v) => patchSearch({ store_id: v })}
            placeholder="按门店筛选"
          />
        </div>
      </DataTableToolbar>

      <DataTable
        tableId="customers"
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
        onRowClick={(row) => {
          void navigate({ to: '/customers/$customerId', params: { customerId: row.id } })
        }}
      />
    </div>
  )
}
