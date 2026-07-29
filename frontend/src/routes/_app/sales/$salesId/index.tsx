import { useQuery } from '@tanstack/react-query'
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'
import { requirePerm } from '@/auth/route-guard'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { customerDetailOptions } from '@/hooks/useCustomers'
import { salesDetailOptions, useVoidSalesRecord } from '@/hooks/useSales'
import { PERFORMANCE_STATUS_LABELS, RECORD_STATUS_LABELS, RECORD_TYPE_LABELS } from '@/lib/labels'
import { formatAmount } from '@/lib/money'
import { notify } from '@/lib/notify'

export const Route = createFileRoute('/_app/sales/$salesId/')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:records:read'),
  component: Page,
})

function Page() {
  const { salesId } = Route.useParams()
  const { data: record } = useQuery(salesDetailOptions(salesId))
  const { data: customer } = useQuery({
    ...customerDetailOptions(record?.customer_id ?? ''),
    enabled: Boolean(record?.customer_id),
  })
  const voidMutation = useVoidSalesRecord()
  const navigate = useNavigate()

  if (!record) return null

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="icon"
          render={<Link to="/sales" aria-label="返回销售记录列表" />}
        >
          <ArrowLeft />
        </Button>
        <h1 className="text-xl font-semibold">
          {RECORD_TYPE_LABELS[record.record_type] ?? record.record_type}
        </h1>
        <Badge variant="outline">{RECORD_STATUS_LABELS[record.status] ?? record.status}</Badge>
        <div className="ml-auto">
          {record.status === 'active' && (
            <Button
              variant="destructive"
              disabled={voidMutation.isPending}
              onClick={() =>
                voidMutation.mutate(record.id, {
                  onSuccess: () => {
                    notify.info('已提交作废审批')
                    void navigate({ to: '/sales' })
                  },
                  onError: (error) => notify.error(error),
                })
              }
            >
              作废
            </Button>
          )}
        </div>
      </div>

      <Card>
        <CardHeader><CardTitle>金额与客户</CardTitle></CardHeader>
        <CardContent className="grid gap-3 md:grid-cols-4">
          <Item label="销售总价" value={formatAmount(record.total_amount)} />
          <Item label="本次实收" value={formatAmount(record.received_amount)} />
          <Item label="欠款变化" value={formatAmount(record.debt_change)} />
          <Item label="客户当前欠款" value={formatAmount(customer?.outstanding_amount ?? '0.00')} />
          <Item label="业务日期" value={record.record_date} />
          <Item
            label="业绩状态"
            value={record.performance_status
              ? PERFORMANCE_STATUS_LABELS[record.performance_status] ?? record.performance_status
              : '不产生业绩'}
          />
          <Item label="客户" value={customer?.name ?? record.customer_id} />
          <Item label="处理人" value={record.handler_user_id} />
        </CardContent>
      </Card>

      {record.lines.length > 0 && (
        <Card>
          <CardHeader><CardTitle>售出内容</CardTitle></CardHeader>
          <CardContent className="flex flex-col gap-2">
            {record.lines.map((line) => (
              <div key={line.id} className="flex items-center gap-3 rounded-md border p-3">
                <span className="font-medium">{line.item_name}</span>
                <span className="text-muted-foreground">
                  {line.operation_total_count ? `${line.operation_total_count} 次` : ''}
                </span>
                <span className="ml-auto text-sm">{line.remark}</span>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      {record.allocations.length > 0 && (
        <Card>
          <CardHeader><CardTitle>业绩分配</CardTitle></CardHeader>
          <CardContent className="flex flex-col gap-2">
            {record.allocations.map((allocation) => (
              <div key={allocation.id} className="flex justify-between rounded-md border p-3">
                <span>{allocation.guide_user_id}</span>
                <span>{allocation.allocation_ratio}% · ¥{formatAmount(allocation.allocated_amount)}</span>
              </div>
            ))}
          </CardContent>
        </Card>
      )}

      {record.remark && (
        <Card>
          <CardHeader><CardTitle>备注</CardTitle></CardHeader>
          <CardContent>{record.remark}</CardContent>
        </Card>
      )}
    </div>
  )
}

function Item({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 font-medium">{value}</div>
    </div>
  )
}
