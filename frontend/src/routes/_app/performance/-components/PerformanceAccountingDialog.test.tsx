import type { ReactNode } from 'react'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PerformanceAccountingDialog } from './PerformanceAccountingDialog'

const exportPerformance = vi.fn()
const postMutate = vi.fn()
const notifySuccess = vi.fn()
const notifyError = vi.fn()
let canPost = true

const record = {
  sales_record_id: 'sale-1',
  received_amount: '100.00',
  record_date: '2026-07-08',
  expert_user_id: 'expert-1',
  expert_amount: '100.00',
  guide_amount: '100.00',
  total_amount: '200.00',
  guide_count: 1,
  system_id: 'system-1',
  store_id: 'store-1',
}

vi.mock('@/hooks/usePerformance', () => ({
  pendingPerformanceOptions: (periodMonth: string) => ({
    queryKey: ['pending', periodMonth],
    queryFn: async () => ({
      sales_records: periodMonth === '2026-07-01' ? [record] : [],
      page_number: 1,
      page_size: 200,
      total_count: periodMonth === '2026-07-01' ? 1 : 0,
    }),
  }),
  performanceBatchesOptions: (periodMonth: string) => ({
    queryKey: ['batches', periodMonth],
    queryFn: async () => ({
      batches: periodMonth === '2026-07-01' ? [{
        id: 'batch-1',
        period_month: '2026-07-01',
        batch_type: 'posting',
        record_count: 1,
        expert_amount: '100.00',
        guide_amount: '100.00',
        total_amount: '200.00',
        posted_by_user_id: 'actor-1',
        posted_at: '2026-07-10T02:00:00Z',
        created_at: '2026-07-10T02:00:00Z',
      }] : [],
      page_number: 1,
      page_size: 50,
      total_count: periodMonth === '2026-07-01' ? 1 : 0,
    }),
  }),
  usePostPerformanceBatch: () => ({ mutate: postMutate, isPending: false }),
  exportPerformance: (...args: unknown[]) => exportPerformance(...args),
}))

vi.mock('@/auth/PermissionProvider', () => ({
  usePermission: () => canPost,
}))

vi.mock('@/lib/notify', () => ({
  notify: {
    success: (...args: unknown[]) => notifySuccess(...args),
    error: (...args: unknown[]) => notifyError(...args),
  },
}))

vi.mock('@/components/UserName', () => ({
  UserName: ({ userId }: { userId?: string }) => <span>{userId ?? '-'}</span>,
}))

vi.mock('@/components/pickers/DatePicker', () => ({
  DatePicker: ({ value, onChange }: { value?: string; onChange: (value?: string) => void }) => (
    <input type="date" value={value ?? ''} onChange={(event) => onChange(event.target.value)} />
  ),
}))

vi.mock('@/components/pickers/UserPicker', () => ({
  UserPicker: ({ value, onChange }: { value?: string; onChange: (value?: string) => void }) => (
    <select aria-label="人员" value={value ?? ''} onChange={(event) => onChange(event.target.value || undefined)}>
      <option value="">全部人员</option><option value="user-1">人员一</option>
    </select>
  ),
}))

vi.mock('@/components/pickers/SystemPicker', () => ({
  SystemPicker: ({ value, onChange }: { value?: string; onChange: (value?: string) => void }) => (
    <select aria-label="体系" value={value ?? ''} onChange={(event) => onChange(event.target.value || undefined)}>
      <option value="">全部体系</option><option value="system-1">体系一</option><option value="system-2">体系二</option>
    </select>
  ),
}))

vi.mock('@/components/pickers/StorePicker', () => ({
  StorePicker: ({ systemId, value, onChange }: { systemId?: string; value?: string; onChange: (value?: string) => void }) => (
    <select aria-label="门店" disabled={!systemId} value={value ?? ''} onChange={(event) => onChange(event.target.value || undefined)}>
      <option value="">全部门店</option><option value="store-1">门店一</option>
    </select>
  ),
}))

vi.mock('@/components/ui/select', () => ({
  Select: ({ items, value, onValueChange, children }: { items: { label: string }[]; value?: string; onValueChange: (value: string) => void; children: ReactNode }) => (
    <select aria-label={items[0]?.label} value={value} onChange={(event) => onValueChange(event.target.value)}>{children}</select>
  ),
  SelectTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
  SelectValue: () => null,
  SelectContent: ({ children }: { children: ReactNode }) => <>{children}</>,
  SelectItem: ({ value, children }: { value: string; children: ReactNode }) => <option value={value}>{children}</option>,
}))

function renderDialog() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={client}>
      <PerformanceAccountingDialog />
    </QueryClientProvider>,
  )
}

describe('PerformanceAccountingDialog', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    vi.setSystemTime(new Date('2026-07-10T08:00:00+08:00'))
    canPost = true
    exportPerformance.mockReset().mockResolvedValue(undefined)
    postMutate.mockReset().mockImplementation((_body, options) => options.onSuccess())
    notifySuccess.mockReset()
    notifyError.mockReset()
    vi.spyOn(window, 'confirm').mockReturnValue(true)
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  it('posts selected sales_records and automatically advances to export and batch results', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    renderDialog()
    await user.click(screen.getByRole('button', { name: '业绩核算' }))

    expect(screen.getByText('1 / 2 · 销售记录入账')).toBeInTheDocument()
    await waitFor(() => expect(screen.getByLabelText('选择销售记录 sale-1')).toBeInTheDocument())
    await user.click(screen.getByLabelText('选择销售记录 sale-1'))
    expect(screen.getByRole('button', { name: '入账并下一步 (1)' })).toBeEnabled()

    await user.click(screen.getByRole('button', { name: '入账并下一步 (1)' }))
    expect(postMutate).toHaveBeenCalledWith(
      { period_month: '2026-07-01', sales_record_ids: ['sale-1'] },
      expect.objectContaining({ onSuccess: expect.any(Function), onError: expect.any(Function) }),
    )
    expect(screen.getByText('2 / 2 · 结果与导出')).toBeInTheDocument()
    expect(screen.getByText('批量入账')).toBeInTheDocument()
    expect(screen.getByText('200.00')).toBeInTheDocument()
  })

  it('syncs the export default to the accounting month and submits extra filters', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    renderDialog()
    await user.click(screen.getByRole('button', { name: '业绩核算' }))
    fireEvent.change(screen.getByLabelText('核算月份'), { target: { value: '2026-08' } })
    await user.click(screen.getByRole('button', { name: '下一步' }))

    expect(screen.getByLabelText('月份快捷选择')).toHaveValue('2026-08')
    await user.selectOptions(screen.getByLabelText('人员'), 'user-1')
    await user.selectOptions(screen.getByLabelText('体系'), 'system-1')
    await user.selectOptions(screen.getByLabelText('门店'), 'store-1')
    await user.selectOptions(screen.getByLabelText('全部角色'), 'guide')
    await user.selectOptions(screen.getByLabelText('全部类型'), 'earning')
    await user.click(screen.getByRole('button', { name: '导出 Excel' }))

    await waitFor(() => expect(exportPerformance).toHaveBeenCalledWith({
      performance_date_from: '2026-08-01',
      performance_date_to: '2026-08-31',
      user_id: 'user-1',
      performance_role: 'guide',
      system_id: 'system-1',
      store_id: 'store-1',
      entry_type: 'earning',
    }))
    expect(notifySuccess).toHaveBeenCalledWith('Excel 已导出')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
  })

  it('allows read-only users to skip posting and keeps export failures open', async () => {
    canPost = false
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    const failure = new Error('export failed')
    exportPerformance.mockRejectedValue(failure)
    renderDialog()
    await user.click(screen.getByRole('button', { name: '业绩核算' }))
    await waitFor(() => expect(screen.getByLabelText('选择销售记录 sale-1')).toHaveAttribute('aria-disabled', 'true'))
    await user.click(screen.getByRole('button', { name: '下一步' }))
    await user.click(screen.getByRole('button', { name: '导出 Excel' }))

    await waitFor(() => expect(notifyError).toHaveBeenCalledWith(failure))
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '上一步' })).toBeEnabled()
  })
})
