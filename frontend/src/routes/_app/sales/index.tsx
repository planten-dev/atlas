import { useState } from 'react'
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
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
import { SystemPicker } from '@/components/pickers/SystemPicker'
import { StorePicker } from '@/components/pickers/StorePicker'
import { UserPicker } from '@/components/pickers/UserPicker'
import { CustomerPicker } from '@/components/pickers/CustomerPicker'
import { DatePicker } from '@/components/pickers/DatePicker'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import { useIsMobile } from '@/hooks/use-mobile'
import { salesListOptions, type SalesRecordResponse } from '@/hooks/useSales'
import { SalesFormDialog } from './-form/SalesFormDialog'
import { customerNameCell } from './-form/cells'
import { formatDate } from '@/lib/date'
import { formatAmount, sumAmounts } from '@/lib/money'
import { RECORD_STATUS_LABELS, RECORD_TYPE_LABELS } from '@/lib/labels'

const searchSchema = z.object({
  status_filter: z.enum(['active', 'voided']).optional(),
  record_type: z.enum(['deal', 'pre_service', 'debt_collection']).optional(),
  customer_id: z.string().optional(),
  system_id: z.string().optional(),
  store_id: z.string().optional(),
  handler_user_id: z.string().optional(),
  record_date_from: z.string().optional(),
  record_date_to: z.string().optional(),
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

type SalesSearch = z.infer<typeof searchSchema>

export const Route = createFileRoute('/_app/sales/')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:records:read'),
  staticData: { tab: 'sales' },
  component: SalesListPage,
})

const columns: ColumnDef<SalesRecordResponse>[] = [
  {
    accessorKey: 'record_date',
    header: '成交日期',
    cell: ({ row }) => formatDate(row.original.record_date),
  },
  {
    accessorKey: 'record_type',
    header: '类型',
    cell: ({ row }) => (
      <Badge variant="outline">
        {RECORD_TYPE_LABELS[row.original.record_type] ?? row.original.record_type}
      </Badge>
    ),
  },
  {
    accessorKey: 'customer_id',
    header: '客户',
    // 移动卡片以客户名为标题
    meta: { card: 'title' },
    cell: ({ row }) => customerNameCell(row.original.customer_id),
  },
  {
    accessorKey: 'handler_user_id',
    header: '处理人',
    cell: ({ row }) => <UserName userId={row.original.handler_user_id} />,
  },
  {
    accessorKey: 'total_amount',
    header: '应收',
    meta: { align: 'right' },
    cell: ({ row }) => (
      <span className="tabular-nums">{formatAmount(row.original.total_amount)}</span>
    ),
  },
  {
    accessorKey: 'received_amount',
    header: '已收',
    meta: { align: 'right' },
    cell: ({ row }) => (
      <span className="tabular-nums">{formatAmount(row.original.received_amount)}</span>
    ),
  },
  {
    accessorKey: 'debt_change',
    header: '未收',
    meta: { align: 'right' },
    cell: ({ row }) => (
      <span className="tabular-nums">{formatAmount(row.original.debt_change)}</span>
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

function SalesListPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const isMobile = useIsMobile()
  const [createOpen, setCreateOpen] = useState(false)
  const query = useQuery(salesListOptions(search))
  const rows = query.data?.items ?? []

  const patchSearch = (patch: Partial<SalesSearch>) => {
    void navigate({ search: (prev) => ({ ...prev, ...patch, page_number: 1 }) })
  }

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">销售记录</h1>

      {!isMobile && <SalesFormDialog open={createOpen} onOpenChange={setCreateOpen} />}

      <DataTableToolbar
        exportConfig={{
          filename: `销售记录-${new Date().toISOString().slice(0, 10)}`,
          columns: [
            { header: '成交日期', value: (r: SalesRecordResponse) => r.record_date },
            { header: '类型', value: (r) => RECORD_TYPE_LABELS[r.record_type] ?? r.record_type },
            { header: '客户ID', value: (r) => r.customer_id },
            { header: '销售总价', value: (r) => formatAmount(r.total_amount) },
            { header: '本次实收', value: (r) => formatAmount(r.received_amount) },
            { header: '欠款变化', value: (r) => formatAmount(r.debt_change) },
            { header: '状态', value: (r) => RECORD_STATUS_LABELS[r.status] ?? r.status },
          ],
          rows,
        }}
        actions={
          <Guard perm="sales:records:write">
            {/* 桌面弹窗;移动端跳分步页(表单过长不适合弹窗) */}
            {isMobile ? (
              <Button render={<Link to="/sales/new" />}>
                <Plus />
                销售录入
              </Button>
            ) : (
              <Button onClick={() => setCreateOpen(true)}>
                <Plus />
                销售录入
              </Button>
            )}
          </Guard>
        }
      >
        <div className="grid w-full gap-2 md:grid-cols-4">
          <CustomerPicker
            value={search.customer_id}
            onChange={(v) => patchSearch({ customer_id: v })}
            allowCreate={false}
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
          <UserPicker
            value={search.handler_user_id}
            onChange={(v) => patchSearch({ handler_user_id: v })}
            placeholder="按处理人筛选"
          />
          <Select
            items={[
              { value: 'all', label: '全部类型' },
              { value: 'sale', label: '销售' },
              { value: 'service', label: '服务' },
            ]}
            value={search.record_type ?? 'all'}
            onValueChange={(value) =>
              patchSearch({
                record_type: value === 'all' ? undefined : (value as 'deal' | 'pre_service' | 'debt_collection'),
              })
            }
          >
            <SelectTrigger className="w-full">
              <SelectValue placeholder="类型" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部类型</SelectItem>
              <SelectItem value="deal">成交</SelectItem>
              <SelectItem value="pre_service">铺垫+服务</SelectItem>
              <SelectItem value="debt_collection">收欠款</SelectItem>
            </SelectContent>
          </Select>
          <div className="flex min-w-0 items-center gap-1">
            <DatePicker
              value={search.record_date_from}
              onChange={(v) => patchSearch({ record_date_from: v })}
              placeholder="成交日期起"
              aria-label="成交日期起"
              className="min-w-0 flex-1"
            />
            <span className="shrink-0 text-muted-foreground">~</span>
            <DatePicker
              value={search.record_date_to}
              onChange={(v) => patchSearch({ record_date_to: v })}
              placeholder="成交日期止"
              aria-label="成交日期止"
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
        tableId="sales"
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
          void navigate({ to: '/sales/$salesId', params: { salesId: row.id } })
        }}
        rowClassName={(row) => (row.status === 'voided' ? 'opacity-60' : undefined)}
      />

      {rows.length > 0 && (
        <p className="text-right text-sm text-muted-foreground">
          本页合计:应收{' '}
          <span className="tabular-nums">{sumAmounts(rows.map((r) => r.total_amount))}</span>
          {' · '}实收 <span className="tabular-nums">{sumAmounts(rows.map((r) => r.received_amount))}</span>
          {' · '}欠款变化{' '}
          <span className="tabular-nums">{sumAmounts(rows.map((r) => r.debt_change))}</span>
        </p>
      )}
    </div>
  )
}
