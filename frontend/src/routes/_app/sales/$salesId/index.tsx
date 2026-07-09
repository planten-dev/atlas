import { useState } from 'react'
import { createFileRoute, Link } from '@tanstack/react-router'
import { useQuery, useSuspenseQuery } from '@tanstack/react-query'
import { ArrowLeft, Plus } from 'lucide-react'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardAction, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Progress } from '@/components/ui/progress'
import { UserName } from '@/components/UserName'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import {
  salesDetailOptions,
  useVoidSalesRecord,
  type SalesRecordLineResponse,
  type SalesRecordResponse,
} from '@/hooks/useSales'
import { useUpdateOperationCount } from '@/hooks/useCounts'
import { usagesListOptions, useVoidUsage } from '@/hooks/useUsages'
import { useVoidPayment, type SalesPaymentResponse } from '@/hooks/usePayments'
import { UsageFormDialog } from '@/components/usages/UsageForm'
import { CollectPaymentDialog } from '@/components/payments/CollectPaymentDialog'
import { customerDetailOptions } from '@/hooks/useCustomers'
import { formatDate, formatDateTime } from '@/lib/date'
import { compareAmounts, formatAmount } from '@/lib/money'
import {
  CUSTOMER_TYPE_LABELS,
  DEAL_TYPE_LABELS,
  PAYMENT_TYPE_LABELS,
  PERFORMANCE_STATUS_LABELS,
  RECORD_TYPE_LABELS,
} from '@/lib/labels'
import { notify } from '@/lib/notify'

export const Route = createFileRoute('/_app/sales/$salesId/')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:records:read'),
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(salesDetailOptions(params.salesId)),
  component: SalesDetailPage,
})

function InfoRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex justify-between gap-4 py-1 text-sm">
      <span className="shrink-0 text-muted-foreground">{label}</span>
      <span className="text-right">{children}</span>
    </div>
  )
}

function SalesDetailPage() {
  const { salesId } = Route.useParams()
  const { data: record } = useSuspenseQuery(salesDetailOptions(salesId))
  const { data: customer } = useQuery(customerDetailOptions(record.customer_id))

  const voidMutation = useVoidSalesRecord()
  const isSale = record.record_type === 'sale'

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon-sm" render={<Link to="/sales" search={{}} aria-label="返回" />}>
          <ArrowLeft />
        </Button>
        <h1 className="flex-1 text-xl font-semibold">
          {isSale ? '销售详情' : '服务详情'}
          <Badge variant="outline" className="ml-2 align-middle">
            {RECORD_TYPE_LABELS[record.record_type] ?? record.record_type}
          </Badge>
        </h1>
        {record.status === 'active' && (
          <Guard perm="sales:records:write">
            <AlertDialog>
              <AlertDialogTrigger render={<Button variant="destructive" size="sm" />}>
                作废
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>作废该记录?</AlertDialogTitle>
                  <AlertDialogDescription>
                    作废将级联作废全部明细行、次数账户与付款记录;存在有效耗用记录时无法作废,请先作废耗用。
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>取消</AlertDialogCancel>
                  <AlertDialogAction
                    onClick={() => {
                      voidMutation.mutate(salesId, {
                        onSuccess: () => notify.success('已作废'),
                        onError: (error) => notify.error(error),
                      })
                    }}
                  >
                    确认作废
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </Guard>
        )}
      </div>

      {record.status === 'voided' && (
        <Alert variant="destructive">
          <AlertTitle>该记录已作废</AlertTitle>
          <AlertDescription>作废记录仅供查询,不参与统计。</AlertDescription>
        </Alert>
      )}

      {/* 基本信息 */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">基本信息</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-x-8 md:grid-cols-2">
          <InfoRow label="客户">{customer?.name ?? '…'}</InfoRow>
          <InfoRow label="成交日期">{formatDate(record.record_date)}</InfoRow>
          <InfoRow label="应收金额">
            <span className="tabular-nums">{formatAmount(record.receivable_amount)}</span>
          </InfoRow>
          <InfoRow label="已收金额">
            <span className="tabular-nums">{formatAmount(record.paid_amount)}</span>
          </InfoRow>
          <InfoRow label="未收金额">
            <span className="tabular-nums font-medium">{formatAmount(record.outstanding_amount)}</span>
          </InfoRow>
          <InfoRow label="客户类型">
            {record.customer_type ? CUSTOMER_TYPE_LABELS[record.customer_type] : '-'}
          </InfoRow>
          <InfoRow label="成交类型">
            {record.deal_type ? DEAL_TYPE_LABELS[record.deal_type] : '-'}
          </InfoRow>
          <InfoRow label="处理人">
            <UserName userId={record.handler_user_id} />
          </InfoRow>
          {record.expert_user_id && (
            <InfoRow label="专家">
              <UserName userId={record.expert_user_id} />
            </InfoRow>
          )}
          {record.consultant_user_id && (
            <InfoRow label="咨询师">
              <UserName userId={record.consultant_user_id} />
            </InfoRow>
          )}
          {record.doctor_user_id && (
            <InfoRow label="医生">
              <UserName userId={record.doctor_user_id} />
            </InfoRow>
          )}
          {record.remark && <InfoRow label="备注">{record.remark}</InfoRow>}
        </CardContent>
      </Card>

      <LinesCard record={record} />
      {isSale && <PaymentsCard record={record} />}
      <UsagesCard record={record} />

      <p className="text-xs text-muted-foreground">
        创建于 {formatDateTime(record.created_at)} · 更新于 {formatDateTime(record.updated_at)}
      </p>
    </div>
  )
}

/* ---------------- 明细行 ---------------- */

function LinesCard({ record }: { record: SalesRecordResponse }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">明细行</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col divide-y">
        {record.lines.map((line) => (
          <LineRow key={line.id} record={record} line={line} />
        ))}
      </CardContent>
    </Card>
  )
}

function LineRow({ record, line }: { record: SalesRecordResponse; line: SalesRecordLineResponse }) {
  const [usageOpen, setUsageOpen] = useState(false)
  const count = line.operation_count
  const canOperate =
    record.status === 'active' && line.status === 'active' && count?.status === 'active'

  return (
    <div className="flex flex-col gap-2 py-3">
      <div className="flex items-center gap-2">
        <p className="flex-1 text-sm font-medium">{line.item_name}</p>
        <span className="text-sm tabular-nums">{formatAmount(line.receivable_amount)}</span>
        {line.status === 'voided' && <Badge variant="destructive">已作废</Badge>}
      </div>
      {line.remark && <p className="text-xs text-muted-foreground">{line.remark}</p>}
      {count && (
        <div className="flex flex-col gap-1">
          <Progress
            value={count.total_count > 0 ? (count.used_count / count.total_count) * 100 : 0}
          />
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span>
              次数:总 {count.total_count} · 已用 {count.used_count} · 剩余{' '}
              <span className="font-medium text-foreground">{count.remaining_count}</span>
            </span>
            {canOperate && (
              <span className="flex gap-1">
                <Guard perm="sales:operation-counts:write">
                  <AdjustCountDialog line={line} />
                </Guard>
                <Guard perm="sales:operation-usages:write">
                  <Button variant="outline" size="xs" onClick={() => setUsageOpen(true)}>
                    <Plus />
                    登记耗用
                  </Button>
                </Guard>
              </span>
            )}
          </div>
        </div>
      )}
      <UsageFormDialog
        open={usageOpen}
        onOpenChange={setUsageOpen}
        lockedSalesRecordId={record.id}
        lockedLineId={line.id}
      />
    </div>
  )
}

function AdjustCountDialog({ line }: { line: SalesRecordLineResponse }) {
  const count = line.operation_count
  const [open, setOpen] = useState(false)
  const [value, setValue] = useState(String(count?.total_count ?? 1))
  const updateMutation = useUpdateOperationCount()
  if (!count) return null

  return (
    <>
      <Button variant="outline" size="xs" onClick={() => setOpen(true)}>
        调整次数
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>调整总次数 · {line.item_name}</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">
            当前已用 {count.used_count} 次,总次数不能低于已用次数。
          </p>
          <Input
            type="number"
            min={Math.max(1, count.used_count)}
            value={value}
            onChange={(e) => setValue(e.target.value)}
          />
          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              disabled={updateMutation.isPending}
              onClick={() => {
                const totalCount = Number(value)
                if (!Number.isInteger(totalCount) || totalCount < 1) {
                  notify.error('请输入不小于 1 的整数')
                  return
                }
                updateMutation.mutate(
                  { salesRecordLineId: line.id, totalCount },
                  {
                    onSuccess: () => {
                      notify.success('总次数已更新')
                      setOpen(false)
                    },
                    onError: (error) => notify.error(error),
                  },
                )
              }}
            >
              保存
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

/* ---------------- 付款记录 ---------------- */

function PaymentsCard({ record }: { record: SalesRecordResponse }) {
  const [collectOpen, setCollectOpen] = useState(false)
  const canCollect =
    record.status === 'active' && compareAmounts(record.outstanding_amount, '0') > 0

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">付款记录</CardTitle>
        {canCollect && (
          <Guard perm="sales:records:write">
            <CardAction>
              <Button variant="outline" size="sm" onClick={() => setCollectOpen(true)}>
                <Plus />
                登记回款
              </Button>
            </CardAction>
          </Guard>
        )}
      </CardHeader>
      <CollectPaymentDialog
        open={collectOpen}
        onOpenChange={setCollectOpen}
        salesRecordId={record.id}
        outstanding={record.outstanding_amount}
      />
      <CardContent>
        {record.payments.length === 0 ? (
          <p className="text-sm text-muted-foreground">暂无付款记录</p>
        ) : (
          <div className="flex flex-col divide-y">
            {record.payments.map((payment) => (
              <PaymentRow key={payment.id} payment={payment} recordActive={record.status === 'active'} />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function PaymentRow({
  payment,
  recordActive,
}: {
  payment: SalesPaymentResponse
  recordActive: boolean
}) {
  const voidMutation = useVoidPayment()
  return (
    <div className="flex flex-col gap-1.5 py-3">
      <div className="flex items-center gap-2 text-sm">
        <Badge variant="outline">{PAYMENT_TYPE_LABELS[payment.payment_type] ?? payment.payment_type}</Badge>
        <span className="tabular-nums font-medium">{formatAmount(payment.paid_amount)}</span>
        <span className="flex-1 text-muted-foreground">{formatDateTime(payment.paid_at)}</span>
        <Badge variant="secondary">
          {PERFORMANCE_STATUS_LABELS[payment.performance_status] ?? payment.performance_status}
        </Badge>
        {payment.status === 'voided' ? (
          <Badge variant="destructive">已作废</Badge>
        ) : (
          recordActive && (
            <Guard perm="sales:records:write">
              <AlertDialog>
                <AlertDialogTrigger render={<Button variant="ghost" size="xs" />}>
                  作废
                </AlertDialogTrigger>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>作废该笔付款?</AlertDialogTitle>
                    <AlertDialogDescription>
                      作废后 {formatAmount(payment.paid_amount)} 将从已收金额中扣除,未收金额相应增加。
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogCancel>取消</AlertDialogCancel>
                    <AlertDialogAction
                      onClick={() => {
                        voidMutation.mutate(payment.id, {
                          onSuccess: () => notify.success('付款已作废'),
                          onError: (error) => notify.error(error),
                        })
                      }}
                    >
                      确认作废
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </Guard>
          )
        )}
      </div>
      <div className="flex flex-wrap gap-x-4 gap-y-0.5 text-xs text-muted-foreground">
        {payment.allocations.map((allocation) => (
          <span key={allocation.id}>
            <UserName userId={allocation.guide_user_id} /> {allocation.allocation_ratio}% ·{' '}
            <span className="tabular-nums">{formatAmount(allocation.allocated_amount)}</span>
          </span>
        ))}
      </div>
      {payment.remark && <p className="text-xs text-muted-foreground">{payment.remark}</p>}
    </div>
  )
}

/* ---------------- 耗用记录 ---------------- */

function UsagesCard({ record }: { record: SalesRecordResponse }) {
  const query = useQuery(usagesListOptions({ sales_record_id: record.id, page_size: 200 }))
  const voidMutation = useVoidUsage()
  const usages = query.data?.items ?? []
  const lineNames = new Map(record.lines.map((line) => [line.id, line.item_name]))

  if (record.record_type !== 'sale') return null

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">耗用记录</CardTitle>
      </CardHeader>
      <CardContent>
        {query.isLoading ? (
          <p className="text-sm text-muted-foreground">加载中…</p>
        ) : usages.length === 0 ? (
          <p className="text-sm text-muted-foreground">暂无耗用记录(在明细行上登记耗用)</p>
        ) : (
          <div className="flex flex-col divide-y">
            {usages.map((usage) => (
              <div key={usage.id} className="flex items-center gap-3 py-2 text-sm">
                <div className="flex-1">
                  <p>
                    {formatDateTime(usage.operated_at)} · <UserName userId={usage.operator_user_id} /> ·{' '}
                    {usage.operation_count} 次
                    <span className="ml-1 text-muted-foreground">
                      ({lineNames.get(usage.sales_record_line_id) ?? '明细行'})
                    </span>
                  </p>
                  {usage.remark && <p className="text-muted-foreground">{usage.remark}</p>}
                </div>
                {usage.status === 'voided' ? (
                  <Badge variant="destructive">已作废</Badge>
                ) : (
                  <Guard perm="sales:operation-usages:write">
                    <AlertDialog>
                      <AlertDialogTrigger render={<Button variant="ghost" size="xs" />}>
                        作废
                      </AlertDialogTrigger>
                      <AlertDialogContent>
                        <AlertDialogHeader>
                          <AlertDialogTitle>作废该耗用记录?</AlertDialogTitle>
                          <AlertDialogDescription>
                            作废后将回退 {usage.operation_count} 次到次数账户。
                          </AlertDialogDescription>
                        </AlertDialogHeader>
                        <AlertDialogFooter>
                          <AlertDialogCancel>取消</AlertDialogCancel>
                          <AlertDialogAction
                            onClick={() => {
                              voidMutation.mutate(usage.id, {
                                onSuccess: () => notify.success('已作废并回退次数'),
                                onError: (error) => notify.error(error),
                              })
                            }}
                          >
                            确认作废
                          </AlertDialogAction>
                        </AlertDialogFooter>
                      </AlertDialogContent>
                    </AlertDialog>
                  </Guard>
                )}
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
