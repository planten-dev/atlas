import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import type { ColumnDef } from '@tanstack/react-table'
import { z } from 'zod'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { DataTable, type PageState } from '@/components/data-table/data-table'
import { DataTableToolbar } from '@/components/data-table/toolbar'
import { DatePicker } from '@/components/pickers/DatePicker'
import { StorePicker } from '@/components/pickers/StorePicker'
import { SystemPicker } from '@/components/pickers/SystemPicker'
import { UserPicker } from '@/components/pickers/UserPicker'
import { UserName } from '@/components/UserName'
import { requirePerm } from '@/auth/route-guard'
import {
  performanceEntriesOptions,
  performanceSummaryOptions,
  type PerformanceEntry,
  type PerformanceFilters,
  type PerformanceSummary,
} from '@/hooks/usePerformance'
import { formatDateTime } from '@/lib/date'
import { formatAmount } from '@/lib/money'
import { PerformanceAccountingDialog } from './-components/PerformanceAccountingDialog'

function currentMonth() {
  const now = new Date()
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`
}

function monthRange(month: string) {
  const [year = new Date().getFullYear(), value = 1] = month.split('-').map(Number)
  const last = new Date(year, value, 0).getDate()
  return { from: `${month}-01`, to: `${month}-${String(last).padStart(2, '0')}` }
}

const initialMonth = currentMonth()
const initialRange = monthRange(initialMonth)
const searchSchema = z.object({
  performance_date_from: z.string().default(initialRange.from),
  performance_date_to: z.string().default(initialRange.to),
  user_id: z.string().optional(),
  performance_role: z.enum(['expert', 'guide']).optional(),
  system_id: z.string().optional(),
  store_id: z.string().optional(),
  entry_type: z.enum(['earning', 'reversal']).optional(),
  summary_page: z.number().int().min(1).default(1),
  summary_size: z.number().int().min(1).max(200).default(20),
  entry_page: z.number().int().min(1).default(1),
  entry_size: z.number().int().min(1).max(200).default(20),
})

type PerformanceSearch = z.infer<typeof searchSchema>

export const Route = createFileRoute('/_app/performance/')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:performance:read'),
  staticData: { tab: 'sales' },
  component: PerformancePage,
})

const summaryColumns: ColumnDef<PerformanceSummary>[] = [
  { accessorKey: 'user_name', header: '人员', cell: ({ row }) => row.original.user_name || <UserName userId={row.original.user_id} /> },
  { accessorKey: 'job_number', header: '工号', cell: ({ row }) => row.original.job_number || '-' },
  { accessorKey: 'expert_amount', header: '专家业绩', meta: { align: 'right' }, cell: ({ row }) => <span className="tabular-nums">{formatAmount(row.original.expert_amount)}</span> },
  { accessorKey: 'guide_amount', header: '美导业绩', meta: { align: 'right' }, cell: ({ row }) => <span className="tabular-nums">{formatAmount(row.original.guide_amount)}</span> },
  { accessorKey: 'reversal_amount', header: '冲销金额', meta: { align: 'right' }, cell: ({ row }) => <span className="tabular-nums text-destructive">{formatAmount(row.original.reversal_amount)}</span> },
  { accessorKey: 'net_amount', header: '净业绩', meta: { align: 'right' }, cell: ({ row }) => <span className="font-medium tabular-nums">{formatAmount(row.original.net_amount)}</span> },
]

const entryColumns: ColumnDef<PerformanceEntry>[] = [
  { accessorKey: 'performance_date', header: '业绩日期' },
  { accessorKey: 'user_name', header: '人员', cell: ({ row }) => row.original.user_name || <UserName userId={row.original.user_id} /> },
  { accessorKey: 'job_number', header: '工号', cell: ({ row }) => row.original.job_number || '-' },
  { accessorKey: 'performance_role', header: '角色', cell: ({ row }) => <Badge variant="outline">{row.original.performance_role === 'expert' ? '专家' : '美导'}</Badge> },
  { accessorKey: 'entry_type', header: '类型', cell: ({ row }) => <Badge variant={row.original.entry_type === 'reversal' ? 'destructive' : 'secondary'}>{row.original.entry_type === 'reversal' ? '冲销' : '正向业绩'}</Badge> },
  { accessorKey: 'amount', header: '金额', meta: { align: 'right' }, cell: ({ row }) => <span className="tabular-nums">{formatAmount(row.original.amount)}</span> },
  { accessorKey: 'allocation_ratio', header: '分配比例', meta: { align: 'right' }, cell: ({ row }) => row.original.allocation_ratio ? `${row.original.allocation_ratio}%` : '-' },
  { accessorKey: 'paid_at', header: '收款时间', cell: ({ row }) => formatDateTime(row.original.paid_at) },
  { accessorKey: 'system_name', header: '体系' },
  { accessorKey: 'store_name', header: '门店' },
  { id: 'sales_record', header: '销售记录', cell: ({ row }) => <Link to="/sales/$salesId" params={{ salesId: row.original.sales_record_id }} className="text-primary hover:underline">查看</Link> },
]

function PerformancePage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const filters: PerformanceFilters = {
    performance_date_from: search.performance_date_from,
    performance_date_to: search.performance_date_to,
    user_id: search.user_id,
    performance_role: search.performance_role,
    system_id: search.system_id,
    store_id: search.store_id,
    entry_type: search.entry_type,
  }
  const summaryQuery = useQuery(performanceSummaryOptions(filters, search.summary_page, search.summary_size))
  const entriesQuery = useQuery(performanceEntriesOptions(filters, search.entry_page, search.entry_size))

  const patchSearch = (patch: Partial<PerformanceSearch>) => {
    void navigate({ search: (previous) => ({ ...previous, ...patch, summary_page: 1, entry_page: 1 }) })
  }

  const setSummaryPage = (page: PageState) => void navigate({ search: (previous) => ({ ...previous, summary_page: page.pageNumber, summary_size: page.pageSize }) })
  const setEntryPage = (page: PageState) => void navigate({ search: (previous) => ({ ...previous, entry_page: page.pageNumber, entry_size: page.pageSize }) })
  const clearFilters = () => patchSearch({ ...initialRange, user_id: undefined, performance_role: undefined, system_id: undefined, store_id: undefined, entry_type: undefined })

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">人员业绩</h1>
        <p className="text-sm text-muted-foreground">按业务发生日期查询专家、美导业绩及冲销明细。</p>
      </div>

      <DataTableToolbar actions={<PerformanceAccountingDialog />}>
        <div className="grid w-full gap-2 md:grid-cols-2 xl:grid-cols-4">
          <input type="month" value={naturalMonth(search.performance_date_from, search.performance_date_to)} onChange={(event) => event.target.value && patchSearch({ performance_date_from: monthRange(event.target.value).from, performance_date_to: monthRange(event.target.value).to })} className="h-9 rounded-md border bg-background px-3 text-sm" aria-label="月份快捷选择" />
          <div className="grid min-w-0 grid-cols-1 items-center gap-1 sm:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] md:col-span-2 xl:col-span-2">
            <DatePicker value={search.performance_date_from} onChange={(value) => value && patchSearch({ performance_date_from: value })} clearable={false} placeholder="起始日期" className="min-w-0" />
            <span className="hidden shrink-0 text-muted-foreground sm:inline">~</span>
            <DatePicker value={search.performance_date_to} onChange={(value) => value && patchSearch({ performance_date_to: value })} clearable={false} placeholder="结束日期" className="min-w-0" />
          </div>
          <UserPicker value={search.user_id} onChange={(value) => patchSearch({ user_id: value })} placeholder="全部人员" />
          <SystemPicker value={search.system_id} onChange={(value) => patchSearch({ system_id: value, store_id: undefined })} placeholder="全部体系" />
          <StorePicker systemId={search.system_id} value={search.store_id} onChange={(value) => patchSearch({ store_id: value })} placeholder="全部门店" />
          <FilterSelect value={search.performance_role} onChange={(value) => patchSearch({ performance_role: value as PerformanceSearch['performance_role'] })} allLabel="全部角色" options={[['expert', '专家'], ['guide', '美导']]} />
          <FilterSelect value={search.entry_type} onChange={(value) => patchSearch({ entry_type: value as PerformanceSearch['entry_type'] })} allLabel="全部类型" options={[['earning', '正向业绩'], ['reversal', '冲销']]} />
          <Button variant="outline" onClick={clearFilters}>清除筛选</Button>
        </div>
      </DataTableToolbar>

      <Card><CardHeader><CardTitle>人员汇总</CardTitle></CardHeader><CardContent><DataTable columns={summaryColumns} data={summaryQuery.data?.summaries ?? []} totalCount={summaryQuery.data?.total_count ?? 0} page={{ pageNumber: search.summary_page, pageSize: search.summary_size }} onPageChange={setSummaryPage} isLoading={summaryQuery.isLoading} onRowClick={(row) => patchSearch({ user_id: row.user_id })} tableId="performance-summary" /></CardContent></Card>
      <Card><CardHeader><CardTitle>业绩明细</CardTitle></CardHeader><CardContent><DataTable columns={entryColumns} data={entriesQuery.data?.entries ?? []} totalCount={entriesQuery.data?.total_count ?? 0} page={{ pageNumber: search.entry_page, pageSize: search.entry_size }} onPageChange={setEntryPage} isLoading={entriesQuery.isLoading} tableId="performance-entries" /></CardContent></Card>

    </div>
  )
}

function naturalMonth(from: string, to: string) {
  const month = from.slice(0, 7)
  const range = monthRange(month)
  return range.from === from && range.to === to ? month : ''
}

function FilterSelect({ value, onChange, allLabel, options }: { value?: string; onChange: (value?: string) => void; allLabel: string; options: [string, string][] }) {
  return <Select items={[{ value: 'all', label: allLabel }, ...options.map(([option, label]) => ({ value: option, label }))]} value={value ?? 'all'} onValueChange={(next) => onChange(!next || next === 'all' ? undefined : next)}><SelectTrigger className="w-full"><SelectValue /></SelectTrigger><SelectContent><SelectItem value="all">{allLabel}</SelectItem>{options.map(([option, label]) => <SelectItem key={option} value={option}>{label}</SelectItem>)}</SelectContent></Select>
}
