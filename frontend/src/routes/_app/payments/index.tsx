import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import type { ColumnDef } from '@tanstack/react-table'
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
import { DatePicker } from '@/components/pickers/DatePicker'
import { requirePerm } from '@/auth/route-guard'
import { paymentsListOptions, type SalesPaymentResponse } from '@/hooks/usePayments'
import { formatDateTime } from '@/lib/date'
import { formatAmount, sumAmounts } from '@/lib/money'
import {
  PAYMENT_TYPE_LABELS,
  PERFORMANCE_STATUS_LABELS,
  RECORD_STATUS_LABELS,
} from '@/lib/labels'

const searchSchema = z.object({
  status_filter: z.enum(['active', 'voided']).optional(),
  payment_type: z.enum(['initial', 'collection']).optional(),
  sales_record_id: z.string().optional(),
  paid_at_from: z.string().optional(),
  paid_at_to: z.string().optional(),
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

type PaymentsSearch = z.infer<typeof searchSchema>

export const Route = createFileRoute('/_app/payments/')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:records:read'),
  staticData: { tab: 'sales' },
  component: PaymentsListPage,
})

const columns: ColumnDef<SalesPaymentResponse>[] = [
  {
    accessorKey: 'paid_at',
    header: '支付时间',
    cell: ({ row }) => formatDateTime(row.original.paid_at),
  },
  {
    accessorKey: 'payment_type',
    header: '类型',
    cell: ({ row }) => (
      <Badge variant="outline">
        {PAYMENT_TYPE_LABELS[row.original.payment_type] ?? row.original.payment_type}
      </Badge>
    ),
  },
  {
    accessorKey: 'paid_amount',
    header: '金额',
    meta: { align: 'right' },
    cell: ({ row }) => (
      <span className="tabular-nums">{formatAmount(row.original.paid_amount)}</span>
    ),
  },
  {
    accessorKey: 'performance_status',
    header: '业绩状态',
    cell: ({ row }) => (
      <Badge variant="secondary">
        {PERFORMANCE_STATUS_LABELS[row.original.performance_status] ??
          row.original.performance_status}
      </Badge>
    ),
  },
  {
    id: 'allocations',
    header: '业绩分配',
    cell: ({ row }) => (
      <span className="flex flex-wrap gap-x-2 text-xs text-muted-foreground">
        {row.original.allocations.map((allocation) => (
          <span key={allocation.id}>
            <UserName userId={allocation.guide_user_id} /> {allocation.allocation_ratio}%
          </span>
        ))}
      </span>
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

function PaymentsListPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(paymentsListOptions(search))
  const rows = query.data?.items ?? []

  const patchSearch = (patch: Partial<PaymentsSearch>) => {
    void navigate({ search: (prev) => ({ ...prev, ...patch, page_number: 1 }) })
  }

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">回款记录</h1>

      <DataTableToolbar
        exportConfig={{
          filename: `回款记录-${new Date().toISOString().slice(0, 10)}`,
          columns: [
            {
              header: '支付时间',
              value: (r: SalesPaymentResponse) => formatDateTime(r.paid_at),
            },
            { header: '类型', value: (r) => PAYMENT_TYPE_LABELS[r.payment_type] ?? r.payment_type },
            { header: '金额', value: (r) => formatAmount(r.paid_amount) },
            {
              header: '业绩状态',
              value: (r) =>
                PERFORMANCE_STATUS_LABELS[r.performance_status] ?? r.performance_status,
            },
            { header: '状态', value: (r) => RECORD_STATUS_LABELS[r.status] ?? r.status },
            { header: '销售记录ID', value: (r) => r.sales_record_id },
          ],
          rows,
        }}
      >
        <div className="grid w-full gap-2 md:grid-cols-4">
          <Select
            items={[
              { value: 'all', label: '全部类型' },
              { value: 'initial', label: '首款' },
              { value: 'collection', label: '回款' },
            ]}
            value={search.payment_type ?? 'all'}
            onValueChange={(value) =>
              patchSearch({
                payment_type: value === 'all' ? undefined : (value as 'initial' | 'collection'),
              })
            }
          >
            <SelectTrigger className="w-full">
              <SelectValue placeholder="类型" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部类型</SelectItem>
              <SelectItem value="initial">首款</SelectItem>
              <SelectItem value="collection">回款</SelectItem>
            </SelectContent>
          </Select>
          <div className="flex min-w-0 items-center gap-1">
            <DatePicker
              value={search.paid_at_from?.slice(0, 10)}
              onChange={(v) => patchSearch({ paid_at_from: v ? `${v}T00:00:00Z` : undefined })}
              placeholder="支付时间起"
              aria-label="支付时间起"
              className="min-w-0 flex-1"
            />
            <span className="shrink-0 text-muted-foreground">~</span>
            <DatePicker
              value={search.paid_at_to?.slice(0, 10)}
              onChange={(v) => patchSearch({ paid_at_to: v ? `${v}T23:59:59Z` : undefined })}
              placeholder="支付时间止"
              aria-label="支付时间止"
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

      {search.sales_record_id && (
        <div className="flex items-center gap-2 rounded-md border bg-muted/50 px-3 py-2 text-sm">
          正在查看单条销售记录的付款
          <Button
            variant="ghost"
            size="xs"
            onClick={() => patchSearch({ sales_record_id: undefined })}
          >
            清除
          </Button>
        </div>
      )}

      <DataTable
        tableId="payments"
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

      {rows.length > 0 && (
        <p className="text-right text-sm text-muted-foreground">
          本页合计:<span className="tabular-nums">{sumAmounts(rows.map((r) => r.paid_amount))}</span>
        </p>
      )}
    </div>
  )
}
