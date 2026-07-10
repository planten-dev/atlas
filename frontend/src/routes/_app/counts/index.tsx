import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import type { ColumnDef } from '@tanstack/react-table'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Progress } from '@/components/ui/progress'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { DataTable } from '@/components/data-table/data-table'
import { DataTableToolbar } from '@/components/data-table/toolbar'
import { requirePerm } from '@/auth/route-guard'
import { countsListOptions, type OperationCountResponse } from '@/hooks/useCounts'
import { RECORD_STATUS_LABELS } from '@/lib/labels'

const searchSchema = z.object({
  status_filter: z.enum(['active', 'voided']).optional(),
  sales_record_id: z.string().optional(),
  // 类别预设入口(业务目录):等后端 operation-counts/list 支持 content_category_id(缺口 #2)
  category: z.string().optional(),
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

export const Route = createFileRoute('/_app/counts/')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:records:read'),
  staticData: { tab: 'home' },
  component: CountsListPage,
})

const columns: ColumnDef<OperationCountResponse>[] = [
  {
    accessorKey: 'sales_record_id',
    header: '销售记录',
    cell: ({ row }) => (
      <span className="font-mono text-xs">{row.original.sales_record_id.slice(0, 8)}…</span>
    ),
  },
  {
    id: 'progress',
    header: '使用进度',
    cell: ({ row }) => {
      const { total_count, used_count } = row.original
      return (
        <div className="flex min-w-32 items-center gap-2">
          <Progress value={total_count > 0 ? (used_count / total_count) * 100 : 0} className="flex-1" />
          <span className="shrink-0 text-xs text-muted-foreground">
            {used_count}/{total_count}
          </span>
        </div>
      )
    },
  },
  {
    accessorKey: 'remaining_count',
    header: '剩余',
    meta: { align: 'right' },
    cell: ({ row }) => (
      <span className="font-medium tabular-nums">{row.original.remaining_count}</span>
    ),
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

function CountsListPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(
    countsListOptions({
      status_filter: search.status_filter,
      sales_record_id: search.sales_record_id,
      page_number: search.page_number,
      page_size: search.page_size,
    }),
  )
  const rows = query.data?.items ?? []

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">剩余查询</h1>

      {search.category && (
        <Alert>
          <AlertDescription>
            按类别筛选({search.category})等待后端支持,当前展示全部次数账户。
          </AlertDescription>
        </Alert>
      )}

      <DataTableToolbar
        exportConfig={{
          filename: `剩余查询-${new Date().toISOString().slice(0, 10)}`,
          columns: [
            { header: '销售记录ID', value: (r: OperationCountResponse) => r.sales_record_id },
            { header: '总次数', value: (r) => r.total_count },
            { header: '已用次数', value: (r) => r.used_count },
            { header: '剩余次数', value: (r) => r.remaining_count },
            { header: '状态', value: (r) => RECORD_STATUS_LABELS[r.status] ?? r.status },
          ],
          rows,
        }}
      >
        <Select
          items={[
            { value: 'all', label: '全部状态' },
            { value: 'active', label: '正常' },
            { value: 'voided', label: '已作废' },
          ]}
          value={search.status_filter ?? 'all'}
          onValueChange={(value) => {
            void navigate({
              search: (prev) => ({
                ...prev,
                status_filter: value === 'all' ? undefined : (value as 'active' | 'voided'),
                page_number: 1,
              }),
            })
          }}
        >
          <SelectTrigger className="w-40">
            <SelectValue placeholder="状态" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">全部状态</SelectItem>
            <SelectItem value="active">正常</SelectItem>
            <SelectItem value="voided">已作废</SelectItem>
          </SelectContent>
        </Select>
      </DataTableToolbar>

      <DataTable
        tableId="counts"
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
      />
    </div>
  )
}
