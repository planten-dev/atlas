import { useState } from 'react'
import { createFileRoute, Link } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { UserName } from '@/components/UserName'
import { requirePerm } from '@/auth/route-guard'
import { usePermission } from '@/auth/PermissionProvider'
import {
  pendingPerformanceOptions,
  performanceBatchesOptions,
  performanceEntriesOptions,
  performanceSummaryOptions,
  usePostPerformanceBatch,
} from '@/hooks/usePerformance'
import { formatDateTime } from '@/lib/date'
import { formatAmount, sumAmounts } from '@/lib/money'
import { notify } from '@/lib/notify'

export const Route = createFileRoute('/_app/performance/')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:performance:read'),
  staticData: { tab: 'sales' },
  component: PerformancePage,
})

function currentMonth(): string {
  const now = new Date()
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`
}

function PerformancePage() {
  const [month, setMonth] = useState(currentMonth)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [detailUserId, setDetailUserId] = useState<string>()
  const periodMonth = `${month}-01`
  const canPost = usePermission('sales:performance:post')
  const pendingQuery = useQuery(pendingPerformanceOptions(periodMonth))
  const summaryQuery = useQuery(performanceSummaryOptions(periodMonth))
  const entriesQuery = useQuery(performanceEntriesOptions(periodMonth, detailUserId))
  const batchesQuery = useQuery(performanceBatchesOptions(periodMonth))
  const postMutation = usePostPerformanceBatch()
  const payments = pendingQuery.data?.payments ?? []

  const selectedPayments = payments.filter((payment) => selected.has(payment.payment_id))
  const selectedExpert = sumAmounts(selectedPayments.map((payment) => payment.expert_amount))
  const selectedGuide = sumAmounts(selectedPayments.map((payment) => payment.guide_amount))
  const selectedTotal = sumAmounts(selectedPayments.map((payment) => payment.total_amount))
  const allSelected = payments.length > 0 && payments.every((payment) => selected.has(payment.payment_id))

  const toggle = (paymentId: string, checked: boolean) => {
    setSelected((current) => {
      const next = new Set(current)
      if (checked) next.add(paymentId)
      else next.delete(paymentId)
      return next
    })
  }

  const postSelected = () => {
    if (selected.size === 0) return
    if (!window.confirm(`确认入账 ${selected.size} 笔收款，总业绩 ${formatAmount(selectedTotal)}？`)) return
    postMutation.mutate(
      { period_month: periodMonth, payment_ids: [...selected] },
      {
        onSuccess: () => {
          setSelected(new Set())
          notify.success('业绩批次已入账')
        },
        onError: notify.error,
      },
    )
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold">人员业绩</h1>
          <p className="text-sm text-muted-foreground">专家诊按收款额额外计算100%，美导按原分配计算100%。</p>
        </div>
        <input
          type="month"
          value={month}
          onChange={(event) => {
            setMonth(event.target.value)
            setSelected(new Set())
            setDetailUserId(undefined)
          }}
          className="h-9 rounded-md border bg-background px-3 text-sm"
          aria-label="核算月份"
        />
      </div>

      <div className="grid gap-3 md:grid-cols-3">
        <Metric title="已选专家业绩" value={selectedExpert} />
        <Metric title="已选美导业绩" value={selectedGuide} />
        <Metric title="已选总业绩" value={selectedTotal} />
      </div>

      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <CardTitle>待入账收款</CardTitle>
          {canPost && (
            <Button onClick={postSelected} disabled={selected.size === 0 || postMutation.isPending}>
              {postMutation.isPending ? '入账中…' : `批量入账 (${selected.size})`}
            </Button>
          )}
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>
                  <Checkbox
                    checked={allSelected}
                    onCheckedChange={(checked) =>
                      setSelected(checked ? new Set(payments.map((payment) => payment.payment_id)) : new Set())
                    }
                    aria-label="全选待入账收款"
                  />
                </TableHead>
                <TableHead>收款时间</TableHead><TableHead>专家</TableHead>
                <TableHead className="text-right">收款</TableHead>
                <TableHead className="text-right">专家业绩</TableHead>
                <TableHead className="text-right">美导业绩</TableHead>
                <TableHead className="text-right">总业绩</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {payments.map((payment) => (
                <TableRow key={payment.payment_id}>
                  <TableCell><Checkbox checked={selected.has(payment.payment_id)} onCheckedChange={(checked) => toggle(payment.payment_id, checked)} aria-label={`选择收款 ${payment.payment_id}`} /></TableCell>
                  <TableCell>{formatDateTime(payment.paid_at)}</TableCell>
                  <TableCell><UserName userId={payment.expert_user_id} /></TableCell>
                  <TableCell className="text-right tabular-nums">{formatAmount(payment.paid_amount)}</TableCell>
                  <TableCell className="text-right tabular-nums">{formatAmount(payment.expert_amount)}</TableCell>
                  <TableCell className="text-right tabular-nums">{formatAmount(payment.guide_amount)}</TableCell>
                  <TableCell className="text-right tabular-nums font-medium">{formatAmount(payment.total_amount)}</TableCell>
                </TableRow>
              ))}
              {!pendingQuery.isLoading && payments.length === 0 && <TableRow><TableCell colSpan={7} className="py-8 text-center text-muted-foreground">本月没有待入账收款</TableCell></TableRow>}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>人员汇总</CardTitle></CardHeader>
        <CardContent>
          <Table>
            <TableHeader><TableRow><TableHead>人员</TableHead><TableHead className="text-right">专家业绩</TableHead><TableHead className="text-right">美导业绩</TableHead><TableHead className="text-right">冲销</TableHead><TableHead className="text-right">净业绩</TableHead></TableRow></TableHeader>
            <TableBody>
              {(summaryQuery.data?.summaries ?? []).map((summary) => (
                <TableRow key={summary.user_id} className="cursor-pointer" onClick={() => setDetailUserId(summary.user_id)}>
                  <TableCell><UserName userId={summary.user_id} /></TableCell>
                  <TableCell className="text-right tabular-nums">{formatAmount(summary.expert_amount)}</TableCell>
                  <TableCell className="text-right tabular-nums">{formatAmount(summary.guide_amount)}</TableCell>
                  <TableCell className="text-right tabular-nums text-destructive">{formatAmount(summary.reversal_amount)}</TableCell>
                  <TableCell className="text-right tabular-nums font-medium">{formatAmount(summary.net_amount)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      {detailUserId && (
        <Card>
          <CardHeader className="flex-row items-center justify-between"><CardTitle><UserName userId={detailUserId} /> 的业绩明细</CardTitle><Button variant="ghost" onClick={() => setDetailUserId(undefined)}>关闭</Button></CardHeader>
          <CardContent>
            <Table>
              <TableHeader><TableRow><TableHead>时间</TableHead><TableHead>角色</TableHead><TableHead>类型</TableHead><TableHead>销售记录</TableHead><TableHead className="text-right">金额</TableHead></TableRow></TableHeader>
              <TableBody>
                {(entriesQuery.data?.entries ?? []).map((entry) => (
                  <TableRow key={entry.id}>
                    <TableCell>{formatDateTime(entry.created_at)}</TableCell>
                    <TableCell><Badge variant="outline">{entry.performance_role === 'expert' ? '专家' : `美导 ${entry.allocation_ratio ?? ''}%`}</Badge></TableCell>
                    <TableCell><Badge variant={entry.entry_type === 'reversal' ? 'destructive' : 'secondary'}>{entry.entry_type === 'reversal' ? '冲销' : '业绩'}</Badge></TableCell>
                    <TableCell><Link to="/sales/$salesId" params={{ salesId: entry.sales_record_id }} className="text-primary hover:underline">查看销售</Link></TableCell>
                    <TableCell className="text-right tabular-nums">{formatAmount(entry.amount)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader><CardTitle>入账批次</CardTitle></CardHeader>
        <CardContent>
          <Table>
            <TableHeader><TableRow><TableHead>入账时间</TableHead><TableHead>类型</TableHead><TableHead>收款笔数</TableHead><TableHead>操作人</TableHead><TableHead className="text-right">总业绩</TableHead></TableRow></TableHeader>
            <TableBody>
              {(batchesQuery.data?.batches ?? []).map((batch) => (
                <TableRow key={batch.id}>
                  <TableCell>{formatDateTime(batch.posted_at)}</TableCell>
                  <TableCell><Badge variant={batch.batch_type === 'reversal' ? 'destructive' : 'secondary'}>{batch.batch_type === 'reversal' ? '自动冲销' : '批量入账'}</Badge></TableCell>
                  <TableCell>{batch.payment_count}</TableCell>
                  <TableCell><UserName userId={batch.posted_by_user_id} /></TableCell>
                  <TableCell className="text-right tabular-nums">{formatAmount(batch.total_amount)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  )
}

function Metric({ title, value }: { title: string; value: string }) {
  return <Card size="sm"><CardHeader><CardTitle>{title}</CardTitle></CardHeader><CardContent className="text-2xl font-semibold tabular-nums">{formatAmount(value)}</CardContent></Card>
}
