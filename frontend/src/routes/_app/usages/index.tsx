import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import type { ColumnDef } from '@tanstack/react-table'
import { Plus } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { DataTable } from '@/components/data-table/data-table'
import { DataTableToolbar } from '@/components/data-table/toolbar'
import { UserName } from '@/components/UserName'
import { UserPicker } from '@/components/pickers/UserPicker'
import { DatePicker } from '@/components/pickers/DatePicker'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import { useIsMobile } from '@/hooks/use-mobile'
import { DesktopOnlyNotice } from '@/layouts/AppShell'
import { usagesListOptions, type OperationUsageResponse } from '@/hooks/useUsages'
import { UsageFormDialog } from '@/components/usages/UsageForm'
import { formatDateTime } from '@/lib/date'
import { RECORD_STATUS_LABELS } from '@/lib/labels'

const searchSchema = z.object({
  status_filter: z.enum(['active', 'voided']).optional(),
  sales_record_id: z.string().optional(),
  operator_user_id: z.string().optional(),
  doctor_user_id: z.string().optional(),
  operated_at_from: z.string().optional(),
  operated_at_to: z.string().optional(),
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

type UsagesSearch = z.infer<typeof searchSchema>

export const Route = createFileRoute('/_app/usages/')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:operation-usages:read'),
  staticData: { desktopOnly: true },
  component: UsagesListPage,
})

const columns: ColumnDef<OperationUsageResponse>[] = [
  {
    accessorKey: 'operated_at',
    header: '操作时间',
    cell: ({ row }) => formatDateTime(row.original.operated_at),
  },
  {
    accessorKey: 'operator_user_id',
    header: '操作人',
    cell: ({ row }) => <UserName userId={row.original.operator_user_id} />,
  },
  {
    accessorKey: 'doctor_user_id',
    header: '医生',
    cell: ({ row }) => <UserName userId={row.original.doctor_user_id} />,
  },
  {
    accessorKey: 'operation_count',
    header: '次数',
    meta: { align: 'right' },
    cell: ({ row }) => <span className="tabular-nums">{row.original.operation_count}</span>,
  },
  {
    accessorKey: 'remark',
    header: '备注',
    cell: ({ row }) => <span className="text-muted-foreground">{row.original.remark ?? '-'}</span>,
  },
  {
    accessorKey: 'status',
    header: '状态',
    cell: ({ row }) =>
      row.original.status === 'voided' ? (
        <Badge variant="destructive">已作废</Badge>
      ) : (
        <Badge variant="outline">正常</Badge>
      ),
  },
]

function UsagesListPage() {
  const isMobile = useIsMobile()
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(usagesListOptions(search))
  const rows = query.data?.items ?? []
  const [createOpen, setCreateOpen] = useState(false)

  if (isMobile) return <DesktopOnlyNotice />

  const patchSearch = (patch: Partial<UsagesSearch>) => {
    void navigate({ search: (prev) => ({ ...prev, ...patch, page_number: 1 }) })
  }

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">耗用记录</h1>

      <UsageFormDialog open={createOpen} onOpenChange={setCreateOpen} />

      <DataTableToolbar
        exportConfig={{
          filename: `耗用记录-${new Date().toISOString().slice(0, 10)}`,
          columns: [
            { header: '操作时间', value: (r: OperationUsageResponse) => formatDateTime(r.operated_at) },
            { header: '销售记录ID', value: (r) => r.sales_record_id },
            { header: '次数', value: (r) => r.operation_count },
            { header: '备注', value: (r) => r.remark },
            { header: '状态', value: (r) => RECORD_STATUS_LABELS[r.status] ?? r.status },
          ],
          rows,
        }}
        actions={
          <Guard perm="sales:operation-usages:write">
            <Button onClick={() => setCreateOpen(true)}>
              <Plus />
              登记耗用
            </Button>
          </Guard>
        }
      >
        <div className="grid w-full gap-2 md:grid-cols-4">
          <UserPicker
            value={search.operator_user_id}
            onChange={(v) => patchSearch({ operator_user_id: v })}
            placeholder="按操作人筛选"
          />
          <UserPicker
            value={search.doctor_user_id}
            onChange={(v) => patchSearch({ doctor_user_id: v })}
            placeholder="按医生筛选"
          />
          <div className="flex min-w-0 items-center gap-1">
            <DatePicker
              value={search.operated_at_from?.slice(0, 10)}
              onChange={(v) =>
                patchSearch({ operated_at_from: v ? `${v}T00:00:00Z` : undefined })
              }
              placeholder="操作时间起"
              aria-label="操作时间起"
              className="min-w-0 flex-1"
            />
            <span className="shrink-0 text-muted-foreground">~</span>
            <DatePicker
              value={search.operated_at_to?.slice(0, 10)}
              onChange={(v) =>
                patchSearch({ operated_at_to: v ? `${v}T23:59:59Z` : undefined })
              }
              placeholder="操作时间止"
              aria-label="操作时间止"
              className="min-w-0 flex-1"
            />
          </div>
          <Select
            items={[
              { value: 'all', label: '全部状态' },
              { value: 'active', label: '正常' },
              { value: 'voided', label: '已作废' },
            ]}
            value={search.status_filter ?? 'all'}
            onValueChange={(value) =>
              patchSearch({
                status_filter: value === 'all' ? undefined : (value as 'active' | 'voided'),
              })
            }
          >
            <SelectTrigger className="w-full">
              <SelectValue placeholder="状态" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部状态</SelectItem>
              <SelectItem value="active">正常</SelectItem>
              <SelectItem value="voided">已作废</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </DataTableToolbar>

      <DataTable
        tableId="usages"
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
          void navigate({ to: '/sales/$salesId', params: { salesId: row.sales_record_id } })
        }}
        rowClassName={(row) => (row.status === 'voided' ? 'opacity-60' : undefined)}
      />
    </div>
  )
}
