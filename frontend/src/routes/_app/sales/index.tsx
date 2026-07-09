import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import type { ColumnDef } from '@tanstack/react-table'
import { Plus } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
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
import { CategoryPicker } from '@/components/pickers/CategoryPicker'
import { UserPicker } from '@/components/pickers/UserPicker'
import { CustomerPicker } from '@/components/pickers/CustomerPicker'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import { salesListOptions, type SalesRecordResponse } from '@/hooks/useSales'
import { activeCategoriesOptions } from '@/hooks/useCategories'
import { customerNameCell } from './-form/cells'
import { formatDate } from '@/lib/date'
import { formatAmount, sumAmounts } from '@/lib/money'
import { RECORD_STATUS_LABELS } from '@/lib/labels'

/** 11 个筛选参数全部进 URL(设计 §7);category 为业务目录预设(按类别名运行时解析)。 */
const searchSchema = z.object({
  status_filter: z.enum(['active', 'voided']).optional(),
  record_group_id: z.string().optional(),
  customer_id: z.string().optional(),
  system_id: z.string().optional(),
  store_id: z.string().optional(),
  handler_user_id: z.string().optional(),
  content_category_id: z.string().optional(),
  category: z.string().optional(),
  sale_date_from: z.string().optional(),
  sale_date_to: z.string().optional(),
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
    accessorKey: 'sale_date',
    header: '成交日期',
    cell: ({ row }) => formatDate(row.original.sale_date),
  },
  {
    accessorKey: 'customer_id',
    header: '客户',
    cell: ({ row }) => customerNameCell(row.original.customer_id),
  },
  {
    accessorKey: 'handler_user_id',
    header: '处理人',
    cell: ({ row }) => <UserName userId={row.original.handler_user_id} />,
  },
  {
    accessorKey: 'paid_amount',
    header: () => <span className="block text-right">已收</span>,
    meta: { title: '已收' },
    cell: ({ row }) => (
      <span className="block text-right tabular-nums">{formatAmount(row.original.paid_amount)}</span>
    ),
  },
  {
    accessorKey: 'unpaid_amount',
    header: () => <span className="block text-right">未收</span>,
    meta: { title: '未收' },
    cell: ({ row }) => (
      <span className="block text-right tabular-nums">{formatAmount(row.original.unpaid_amount)}</span>
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
  // 业务目录 ?category=<类别名> 预设:运行时按名解析为 content_category_id
  const { data: categories } = useQuery(activeCategoriesOptions)
  const presetCategoryId = search.category
    ? categories?.find((c) => c.category_name === search.category)?.id
    : undefined
  const { category: _category, ...apiSearch } = search
  void _category
  const query = useQuery(
    salesListOptions({
      ...apiSearch,
      content_category_id: search.content_category_id ?? presetCategoryId,
    }),
  )
  const rows = query.data?.items ?? []

  const patchSearch = (patch: Partial<SalesSearch>) => {
    void navigate({ search: (prev) => ({ ...prev, ...patch, page_number: 1 }) })
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">销售记录</h1>
        <Guard perm="sales:records:write">
          <Button render={<Link to="/sales/new" />}>
            <Plus />
            销售录入
          </Button>
        </Guard>
      </div>

      <DataTableToolbar
        exportConfig={{
          filename: `销售记录-${new Date().toISOString().slice(0, 10)}`,
          columns: [
            { header: '成交日期', value: (r: SalesRecordResponse) => r.sale_date },
            { header: '客户ID', value: (r) => r.customer_id },
            { header: '已收金额', value: (r) => formatAmount(r.paid_amount) },
            { header: '未收金额', value: (r) => formatAmount(r.unpaid_amount) },
            { header: '状态', value: (r) => RECORD_STATUS_LABELS[r.status] ?? r.status },
            { header: '批次', value: (r) => r.record_group_id },
          ],
          rows,
        }}
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
          <CategoryPicker
            value={search.content_category_id}
            onChange={(v) => patchSearch({ content_category_id: v })}
            placeholder="按内容类型筛选"
          />
          <div className="flex items-center gap-1">
            <Input
              type="date"
              value={search.sale_date_from ?? ''}
              onChange={(e) => patchSearch({ sale_date_from: e.target.value || undefined })}
              aria-label="成交日期起"
            />
            <span className="text-muted-foreground">~</span>
            <Input
              type="date"
              value={search.sale_date_to ?? ''}
              onChange={(e) => patchSearch({ sale_date_to: e.target.value || undefined })}
              aria-label="成交日期止"
            />
          </div>
          <Select
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

      {search.record_group_id && (
        <div className="flex items-center gap-2 rounded-md border bg-muted/50 px-3 py-2 text-sm">
          正在查看同批次记录
          <Button
            variant="ghost"
            size="xs"
            onClick={() => patchSearch({ record_group_id: undefined })}
          >
            清除
          </Button>
        </div>
      )}

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
          本页合计:已收 <span className="tabular-nums">{sumAmounts(rows.map((r) => r.paid_amount))}</span>
          {' · '}未收 <span className="tabular-nums">{sumAmounts(rows.map((r) => r.unpaid_amount))}</span>
        </p>
      )}
    </div>
  )
}
