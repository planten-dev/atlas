import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Download } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { DatePicker } from '@/components/pickers/DatePicker'
import { StorePicker } from '@/components/pickers/StorePicker'
import { SystemPicker } from '@/components/pickers/SystemPicker'
import { UserPicker } from '@/components/pickers/UserPicker'
import { UserName } from '@/components/UserName'
import { usePermission } from '@/auth/PermissionProvider'
import {
  exportPerformance,
  pendingPerformanceOptions,
  performanceBatchesOptions,
  type PendingPerformancePayment,
  type PerformanceFilters,
  usePostPerformanceBatch,
} from '@/hooks/usePerformance'
import { formatDateTime } from '@/lib/date'
import { formatAmount, sumAmounts } from '@/lib/money'
import { notify } from '@/lib/notify'

const STEPS = ['收款入账', '结果与导出'] as const

function currentMonth() {
  const now = new Date()
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`
}

function monthRange(month: string) {
  const [year = new Date().getFullYear(), value = 1] = month.split('-').map(Number)
  const last = new Date(year, value, 0).getDate()
  return { from: `${month}-01`, to: `${month}-${String(last).padStart(2, '0')}` }
}

function defaultFilters(month: string): PerformanceFilters {
  const range = monthRange(month)
  return {
    performance_date_from: range.from,
    performance_date_to: range.to,
  }
}

function naturalMonth(from: string, to: string) {
  const month = from.slice(0, 7)
  const range = monthRange(month)
  return range.from === from && range.to === to ? month : ''
}

export function PerformanceAccountingDialog() {
  const [open, setOpen] = useState(false)
  const [step, setStep] = useState(0)
  const [month, setMonth] = useState(currentMonth)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [exporting, setExporting] = useState(false)
  const [filters, setFilters] = useState<PerformanceFilters>(() => defaultFilters(currentMonth()))
  const canPost = usePermission('sales:performance:post')
  const periodMonth = `${month}-01`
  const pendingQuery = useQuery({ ...pendingPerformanceOptions(periodMonth), enabled: open })
  const batchesQuery = useQuery({ ...performanceBatchesOptions(periodMonth), enabled: open })
  const postMutation = usePostPerformanceBatch()
  const payments = pendingQuery.data?.payments ?? []
  const selectedPayments = payments.filter((payment) => selected.has(payment.payment_id))
  const selectedExpert = sumAmounts(selectedPayments.map((payment) => payment.expert_amount))
  const selectedGuide = sumAmounts(selectedPayments.map((payment) => payment.guide_amount))
  const selectedTotal = sumAmounts(selectedPayments.map((payment) => payment.total_amount))
  const allSelected = payments.length > 0 && payments.every((payment) => selected.has(payment.payment_id))
  const busy = exporting || postMutation.isPending

  const handleOpenChange = (nextOpen: boolean) => {
    if (busy && !nextOpen) return
    if (nextOpen) {
      const nextMonth = currentMonth()
      setStep(0)
      setMonth(nextMonth)
      setSelected(new Set())
      setFilters(defaultFilters(nextMonth))
    }
    setOpen(nextOpen)
  }

  const changeMonth = (nextMonth: string) => {
    if (!nextMonth) return
    setMonth(nextMonth)
    setSelected(new Set())
    setFilters(defaultFilters(nextMonth))
  }

  const updateFilters = (patch: Partial<PerformanceFilters>) => {
    setFilters((current) => ({ ...current, ...patch }))
  }

  const togglePayment = (paymentId: string, checked: boolean) => {
    setSelected((current) => {
      const next = new Set(current)
      if (checked) next.add(paymentId)
      else next.delete(paymentId)
      return next
    })
  }

  const goNext = () => {
    if (!canPost || selected.size === 0) {
      setStep(1)
      return
    }
    if (!window.confirm(`确认入账 ${selected.size} 笔收款，总业绩 ${formatAmount(selectedTotal)}？`)) {
      return
    }
    postMutation.mutate(
      { period_month: periodMonth, payment_ids: [...selected] },
      {
        onSuccess: () => {
          setSelected(new Set())
          setStep(1)
          notify.success('业绩批次已入账')
        },
        onError: notify.error,
      },
    )
  }

  const download = async () => {
    setExporting(true)
    try {
      await exportPerformance(filters)
      notify.success('Excel 已导出')
      setOpen(false)
    } catch (error) {
      notify.error(error)
    } finally {
      setExporting(false)
    }
  }

  return (
    <>
      <Button variant="outline" onClick={() => handleOpenChange(true)}>
        业绩核算
      </Button>
      <Dialog open={open} onOpenChange={handleOpenChange}>
        <DialogContent
          className="grid max-h-[90vh] grid-rows-[auto_auto_minmax(0,1fr)_auto] gap-4 sm:max-w-6xl"
          showCloseButton={!busy}
        >
          <DialogHeader>
            <div className="flex items-center justify-between gap-4 pr-8">
              <DialogTitle>业绩核算</DialogTitle>
              <span className="text-sm text-muted-foreground">
                {step + 1} / {STEPS.length} · {STEPS[step]}
              </span>
            </div>
            <DialogDescription>
              先核对并入账有效收款，再查看结果并导出业绩报表。
            </DialogDescription>
          </DialogHeader>

          <div className="flex gap-1">
            {STEPS.map((title, index) => (
              <div
                key={title}
                className={`h-1 flex-1 rounded-full ${index <= step ? 'bg-primary' : 'bg-muted'}`}
              />
            ))}
          </div>

          <div className="-mx-2 min-h-0 overflow-y-auto px-2">
            {step === 0 ? (
              <PostingStep
                month={month}
                onMonthChange={changeMonth}
                payments={payments}
                selected={selected}
                allSelected={allSelected}
                canPost={canPost}
                isLoading={pendingQuery.isLoading}
                selectedExpert={selectedExpert}
                selectedGuide={selectedGuide}
                selectedTotal={selectedTotal}
                onToggle={togglePayment}
                onToggleAll={(checked) =>
                  setSelected(checked ? new Set(payments.map((payment) => payment.payment_id)) : new Set())
                }
              />
            ) : (
              <ExportStep
                filters={filters}
                onFiltersChange={updateFilters}
                batches={batchesQuery.data?.batches ?? []}
                batchesLoading={batchesQuery.isLoading}
              />
            )}
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => handleOpenChange(false)} disabled={busy}>
              取消
            </Button>
            {step > 0 && (
              <Button variant="outline" onClick={() => setStep(0)} disabled={busy}>
                上一步
              </Button>
            )}
            {step === 0 ? (
              <Button onClick={goNext} disabled={postMutation.isPending}>
                {postMutation.isPending
                  ? '入账中…'
                  : canPost && selected.size > 0
                    ? `入账并下一步 (${selected.size})`
                    : '下一步'}
              </Button>
            ) : (
              <Button onClick={() => void download()} disabled={exporting}>
                <Download />
                {exporting ? '生成中…' : '导出 Excel'}
              </Button>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

function PostingStep({
  month,
  onMonthChange,
  payments,
  selected,
  allSelected,
  canPost,
  isLoading,
  selectedExpert,
  selectedGuide,
  selectedTotal,
  onToggle,
  onToggleAll,
}: {
  month: string
  onMonthChange: (month: string) => void
  payments: PendingPerformancePayment[]
  selected: Set<string>
  allSelected: boolean
  canPost: boolean
  isLoading: boolean
  selectedExpert: string
  selectedGuide: string
  selectedTotal: string
  onToggle: (paymentId: string, checked: boolean) => void
  onToggleAll: (checked: boolean) => void
}) {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h3 className="font-medium">待入账收款</h3>
          <p className="text-sm text-muted-foreground">选择本月有效收款并确认生成业绩分录。</p>
        </div>
        <input
          type="month"
          value={month}
          onChange={(event) => onMonthChange(event.target.value)}
          className="h-9 rounded-md border bg-background px-3 text-sm"
          aria-label="核算月份"
        />
      </div>

      <div className="grid gap-3 sm:grid-cols-3">
        <Metric title="已选专家业绩" value={selectedExpert} />
        <Metric title="已选美导业绩" value={selectedGuide} />
        <Metric title="已选总业绩" value={selectedTotal} />
      </div>

      <div className="overflow-x-auto rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>
                <Checkbox
                  checked={allSelected}
                  disabled={!canPost}
                  onCheckedChange={(checked) => onToggleAll(Boolean(checked))}
                  aria-label="全选待入账收款"
                />
              </TableHead>
              <TableHead>收款时间</TableHead>
              <TableHead>专家</TableHead>
              <TableHead className="text-right">收款</TableHead>
              <TableHead className="text-right">专家业绩</TableHead>
              <TableHead className="text-right">美导业绩</TableHead>
              <TableHead className="text-right">总业绩</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {payments.map((payment) => (
              <TableRow key={payment.payment_id}>
                <TableCell>
                  <Checkbox
                    checked={selected.has(payment.payment_id)}
                    disabled={!canPost}
                    onCheckedChange={(checked) => onToggle(payment.payment_id, Boolean(checked))}
                    aria-label={`选择收款 ${payment.payment_id}`}
                  />
                </TableCell>
                <TableCell>{formatDateTime(payment.paid_at)}</TableCell>
                <TableCell><UserName userId={payment.expert_user_id ?? undefined} /></TableCell>
                <TableCell className="text-right tabular-nums">{formatAmount(payment.paid_amount)}</TableCell>
                <TableCell className="text-right tabular-nums">{formatAmount(payment.expert_amount)}</TableCell>
                <TableCell className="text-right tabular-nums">{formatAmount(payment.guide_amount)}</TableCell>
                <TableCell className="text-right font-medium tabular-nums">{formatAmount(payment.total_amount)}</TableCell>
              </TableRow>
            ))}
            {!isLoading && payments.length === 0 && (
              <TableRow>
                <TableCell colSpan={7} className="py-8 text-center text-muted-foreground">
                  该月份没有待入账收款
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  )
}

function ExportStep({
  filters,
  onFiltersChange,
  batches,
  batchesLoading,
}: {
  filters: PerformanceFilters
  onFiltersChange: (patch: Partial<PerformanceFilters>) => void
  batches: Array<{
    id: string
    posted_at: string
    batch_type: string
    payment_count: number
    posted_by_user_id?: string | null
    total_amount: string
  }>
  batchesLoading: boolean
}) {
  return (
    <div className="flex flex-col gap-5">
      <section className="flex flex-col gap-3">
        <div>
          <h3 className="font-medium">导出设置</h3>
          <p className="text-sm text-muted-foreground">默认使用核算月份，可按需要细化本次导出范围。</p>
        </div>
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          <label className="flex flex-col gap-1.5 text-sm font-medium">
            月份快捷选择
            <input
              type="month"
              value={naturalMonth(filters.performance_date_from, filters.performance_date_to)}
              onChange={(event) => {
                if (!event.target.value) return
                const range = monthRange(event.target.value)
                onFiltersChange({
                  performance_date_from: range.from,
                  performance_date_to: range.to,
                })
              }}
              className="h-9 rounded-md border bg-background px-3 text-sm font-normal"
            />
          </label>
          <div className="grid min-w-0 grid-cols-1 gap-2 sm:col-span-2 sm:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] sm:items-end lg:col-span-2">
            <label className="flex min-w-0 flex-col gap-1.5 text-sm font-medium">
              起始日期
              <DatePicker
                value={filters.performance_date_from}
                onChange={(value) => value && onFiltersChange({ performance_date_from: value })}
                clearable={false}
                className="min-w-0"
              />
            </label>
            <span className="hidden h-9 items-center text-muted-foreground sm:flex">~</span>
            <label className="flex min-w-0 flex-col gap-1.5 text-sm font-medium">
              结束日期
              <DatePicker
                value={filters.performance_date_to}
                onChange={(value) => value && onFiltersChange({ performance_date_to: value })}
                clearable={false}
                className="min-w-0"
              />
            </label>
          </div>
          <UserPicker value={filters.user_id} onChange={(value) => onFiltersChange({ user_id: value })} placeholder="全部人员" />
          <FilterSelect value={filters.performance_role} onChange={(value) => onFiltersChange({ performance_role: value as PerformanceFilters['performance_role'] })} allLabel="全部角色" options={[["expert", "专家"], ["guide", "美导"]]} />
          <SystemPicker value={filters.system_id} onChange={(value) => onFiltersChange({ system_id: value, store_id: undefined })} placeholder="全部体系" />
          <StorePicker systemId={filters.system_id} value={filters.store_id} onChange={(value) => onFiltersChange({ store_id: value })} placeholder="全部门店" />
          <FilterSelect value={filters.entry_type} onChange={(value) => onFiltersChange({ entry_type: value as PerformanceFilters['entry_type'] })} allLabel="全部类型" options={[["earning", "正向业绩"], ["reversal", "冲销"]]} />
        </div>
      </section>

      <section className="flex flex-col gap-3">
        <div>
          <h3 className="font-medium">入账批次</h3>
          <p className="text-sm text-muted-foreground">当前核算月份的入账及自动冲销记录。</p>
        </div>
        <div className="overflow-x-auto rounded-md border">
          <Table>
            <TableHeader>
              <TableRow><TableHead>入账时间</TableHead><TableHead>类型</TableHead><TableHead>收款笔数</TableHead><TableHead>操作人</TableHead><TableHead className="text-right">总业绩</TableHead></TableRow>
            </TableHeader>
            <TableBody>
              {batches.map((batch) => (
                <TableRow key={batch.id}>
                  <TableCell>{formatDateTime(batch.posted_at)}</TableCell>
                  <TableCell><Badge variant={batch.batch_type === 'reversal' ? 'destructive' : 'secondary'}>{batch.batch_type === 'reversal' ? '自动冲销' : '批量入账'}</Badge></TableCell>
                  <TableCell>{batch.payment_count}</TableCell>
                  <TableCell><UserName userId={batch.posted_by_user_id ?? undefined} /></TableCell>
                  <TableCell className="text-right tabular-nums">{formatAmount(batch.total_amount)}</TableCell>
                </TableRow>
              ))}
              {!batchesLoading && batches.length === 0 && (
                <TableRow><TableCell colSpan={5} className="py-8 text-center text-muted-foreground">该月份暂无入账批次</TableCell></TableRow>
              )}
            </TableBody>
          </Table>
        </div>
      </section>
    </div>
  )
}

function Metric({ title, value }: { title: string; value: string }) {
  return <Card size="sm"><CardHeader><CardTitle>{title}</CardTitle></CardHeader><CardContent className="text-2xl font-semibold tabular-nums">{formatAmount(value)}</CardContent></Card>
}

function FilterSelect({ value, onChange, allLabel, options }: { value?: string; onChange: (value?: string) => void; allLabel: string; options: [string, string][] }) {
  return <Select items={[{ value: 'all', label: allLabel }, ...options.map(([option, label]) => ({ value: option, label }))]} value={value ?? 'all'} onValueChange={(next) => onChange(!next || next === 'all' ? undefined : next)}><SelectTrigger className="w-full"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="all">{allLabel}</SelectItem>{options.map(([option, label]) => <SelectItem key={option} value={option}>{label}</SelectItem>)}</SelectContent></Select>
}
