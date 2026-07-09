import { useState } from 'react'
import { createFileRoute, Link } from '@tanstack/react-router'
import { useQuery, useSuspenseQuery } from '@tanstack/react-query'
import { ArrowLeft, Pencil, Plus } from 'lucide-react'
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
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
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
import { salesDetailOptions, useVoidSalesRecord } from '@/hooks/useSales'
import { useUpdateOperationCount } from '@/hooks/useCounts'
import { usagesListOptions, useVoidUsage } from '@/hooks/useUsages'
import { customerDetailOptions } from '@/hooks/useCustomers'
import { activeCategoriesOptions } from '@/hooks/useCategories'
import { formatDate, formatDateTime } from '@/lib/date'
import { formatAmount } from '@/lib/money'
import {
  COLLABORATION_TYPE_LABELS,
  CUSTOMER_TYPE_LABELS,
  DEAL_STATUS_LABELS,
  DEAL_TYPE_LABELS,
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
  const { data: categories } = useQuery(activeCategoriesOptions)
  const categoryName =
    categories?.find((c) => c.id === record.content_category_id)?.category_name ??
    record.content_category_id

  const voidMutation = useVoidSalesRecord()

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon-sm" render={<Link to="/sales" search={{}} aria-label="返回" />}>
          <ArrowLeft />
        </Button>
        <h1 className="flex-1 text-xl font-semibold">销售详情</h1>
        {record.status === 'active' && (
          <>
            <Guard perm="sales:records:write">
              <Button
                variant="outline"
                size="sm"
                render={<Link to="/sales/$salesId/edit" params={{ salesId }} />}
              >
                <Pencil />
                编辑
              </Button>
            </Guard>
            <Guard perm="sales:records:write">
              <AlertDialog>
                <AlertDialogTrigger render={<Button variant="destructive" size="sm" />}>
                  作废
                </AlertDialogTrigger>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>作废该销售记录?</AlertDialogTitle>
                    <AlertDialogDescription>
                      作废后记录不可恢复,关联的次数账户也将一并作废。
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
          </>
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
          <InfoRow label="成交日期">{formatDate(record.sale_date)}</InfoRow>
          <InfoRow label="内容类型">{categoryName}</InfoRow>
          <InfoRow label="已收金额">
            <span className="tabular-nums">{formatAmount(record.paid_amount)}</span>
          </InfoRow>
          <InfoRow label="未收金额">
            <span className="tabular-nums">{formatAmount(record.unpaid_amount)}</span>
          </InfoRow>
          <InfoRow label="客户类型">{CUSTOMER_TYPE_LABELS[record.customer_type]}</InfoRow>
          <InfoRow label="成交类型">{DEAL_TYPE_LABELS[record.deal_type]}</InfoRow>
          <InfoRow label="成交状态">{DEAL_STATUS_LABELS[record.deal_status]}</InfoRow>
          <InfoRow label="协作类型">{COLLABORATION_TYPE_LABELS[record.collaboration_type]}</InfoRow>
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
          {record.record_group_id && (
            <InfoRow label="同批次">
              <Link
                to="/sales"
                search={{ record_group_id: record.record_group_id }}
                className="text-primary underline underline-offset-4"
              >
                查看同批次记录
              </Link>
            </InfoRow>
          )}
        </CardContent>
      </Card>

      {/* 次数账户 */}
      {record.operation_count && (
        <CountsCard
          salesRecordId={salesId}
          total={record.operation_count.total_count}
          used={record.operation_count.used_count}
          remaining={record.operation_count.remaining_count}
          voided={record.operation_count.status === 'voided'}
        />
      )}

      {/* 耗用记录 */}
      <UsagesCard salesRecordId={salesId} hasCounts={Boolean(record.operation_count)} />

      <p className="text-xs text-muted-foreground">
        创建于 {formatDateTime(record.created_at)} · 更新于 {formatDateTime(record.updated_at)}
      </p>
    </div>
  )
}

function CountsCard({
  salesRecordId,
  total,
  used,
  remaining,
  voided,
}: {
  salesRecordId: string
  total: number
  used: number
  remaining: number
  voided: boolean
}) {
  const [open, setOpen] = useState(false)
  const [value, setValue] = useState(String(total))
  const updateMutation = useUpdateOperationCount()

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle className="text-base">次数账户</CardTitle>
        {!voided && (
          <Guard perm="sales:operation-counts:write">
            <Button variant="outline" size="sm" onClick={() => setOpen(true)}>
              调整总次数
            </Button>
          </Guard>
        )}
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {voided && <Badge variant="destructive">已作废</Badge>}
        <Progress value={total > 0 ? (used / total) * 100 : 0} />
        <div className="flex justify-between text-sm">
          <span>总 {total} 次</span>
          <span>已用 {used} 次</span>
          <span className="font-medium">剩余 {remaining} 次</span>
        </div>
      </CardContent>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>调整总次数</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">
            当前已用 {used} 次,总次数不能低于已用次数。
          </p>
          <Input
            type="number"
            min={Math.max(1, used)}
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
                  { salesRecordId, totalCount },
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
    </Card>
  )
}

function UsagesCard({ salesRecordId, hasCounts }: { salesRecordId: string; hasCounts: boolean }) {
  const query = useQuery(usagesListOptions({ sales_record_id: salesRecordId, page_size: 200 }))
  const voidMutation = useVoidUsage()
  const usages = query.data?.items ?? []

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle className="text-base">耗用记录</CardTitle>
        {hasCounts && (
          <Guard perm="sales:operation-usages:write">
            <Button
              variant="outline"
              size="sm"
              render={<Link to="/usages/new" search={{ salesRecordId }} />}
            >
              <Plus />
              登记耗用
            </Button>
          </Guard>
        )}
      </CardHeader>
      <CardContent>
        {query.isLoading ? (
          <p className="text-sm text-muted-foreground">加载中…</p>
        ) : usages.length === 0 ? (
          <p className="text-sm text-muted-foreground">暂无耗用记录</p>
        ) : (
          <div className="flex flex-col divide-y">
            {usages.map((usage) => (
              <div key={usage.id} className="flex items-center gap-3 py-2 text-sm">
                <div className="flex-1">
                  <p>
                    {formatDateTime(usage.operated_at)} · <UserName userId={usage.operator_user_id} /> ·{' '}
                    {usage.operation_count} 次
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
