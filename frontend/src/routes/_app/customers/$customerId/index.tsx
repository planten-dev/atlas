import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useQuery, useSuspenseQuery } from '@tanstack/react-query'
import type { ColumnDef } from '@tanstack/react-table'
import { ArrowLeft, Pencil } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { DataTable } from '@/components/data-table/data-table'
import { UserName } from '@/components/UserName'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import { customerDetailOptions } from '@/hooks/useCustomers'
import { salesListOptions, type SalesRecordResponse } from '@/hooks/useSales'
import { formatDate } from '@/lib/date'
import { formatAmount } from '@/lib/money'
import { useState } from 'react'

export const Route = createFileRoute('/_app/customers/$customerId/')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'customers:read'),
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(customerDetailOptions(params.customerId)),
  component: CustomerDetailPage,
})

const salesColumns: ColumnDef<SalesRecordResponse>[] = [
  {
    accessorKey: 'sale_date',
    header: '成交日期',
    cell: ({ row }) => formatDate(row.original.sale_date),
  },
  {
    accessorKey: 'paid_amount',
    header: '已收',
    meta: { align: 'right' },
    cell: ({ row }) => (
      <span className="tabular-nums">{formatAmount(row.original.paid_amount)}</span>
    ),
  },
  {
    accessorKey: 'unpaid_amount',
    header: '未收',
    meta: { align: 'right' },
    cell: ({ row }) => (
      <span className="tabular-nums">{formatAmount(row.original.unpaid_amount)}</span>
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

function CustomerDetailPage() {
  const { customerId } = Route.useParams()
  const navigate = useNavigate()
  const { data: customer } = useSuspenseQuery(customerDetailOptions(customerId))
  const [page, setPage] = useState({ pageNumber: 1, pageSize: 20 })
  const salesQuery = useQuery(
    salesListOptions({
      customer_id: customerId,
      page_number: page.pageNumber,
      page_size: page.pageSize,
    }),
  )

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon-sm" render={<Link to="/customers" search={{}} aria-label="返回" />}>
          <ArrowLeft />
        </Button>
        <h1 className="flex-1 text-xl font-semibold">{customer.name}</h1>
        {customer.status === 'disabled' && <Badge variant="destructive">已停用</Badge>}
        <Guard perm="customers:write">
          <Button
            variant="outline"
            size="sm"
            render={<Link to="/customers/$customerId/edit" params={{ customerId }} />}
          >
            <Pencil />
            编辑
          </Button>
        </Guard>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">客户资料</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-1 text-sm">
          <p>
            <span className="text-muted-foreground">创建人:</span>
            <UserName userId={customer.creator_user_id} />
          </p>
          <p>
            <span className="text-muted-foreground">备注:</span>
            {customer.remark ?? '-'}
          </p>
          <p>
            <span className="text-muted-foreground">创建时间:</span>
            {formatDate(customer.created_at)}
          </p>
          <div className="mt-2 rounded-md border border-dashed p-3 text-muted-foreground">
            附件功能建设中(等待文件服务上线)
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">销售记录</CardTitle>
        </CardHeader>
        <CardContent>
          <DataTable
            tableId="customer-sales"
            columns={salesColumns}
            data={salesQuery.data?.items ?? []}
            totalCount={salesQuery.data?.totalCount ?? 0}
            isLoading={salesQuery.isLoading}
            page={page}
            onPageChange={setPage}
            onRowClick={(row) => {
              void navigate({ to: '/sales/$salesId', params: { salesId: row.id } })
            }}
            emptyText="该客户暂无销售记录"
          />
        </CardContent>
      </Card>
    </div>
  )
}
