import { afterEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ColumnDef } from '@tanstack/react-table'
import { Badge } from '@/components/ui/badge'
import { DataTable } from './data-table'

interface Row {
  name: string
  amount: string
  status: string
  secret: string
}

const rows: Row[] = [
  { name: '张三', amount: '100.00', status: 'active', secret: 'x' },
  { name: '李四', amount: '200.00', status: 'voided', secret: 'y' },
]

function makeColumns(onAction?: () => void): ColumnDef<Row>[] {
  return [
    { accessorKey: 'name', header: '姓名' },
    { accessorKey: 'amount', header: '金额', meta: { align: 'right' } },
    {
      accessorKey: 'status',
      header: '状态',
      cell: ({ row }) => (
        <Badge variant={row.original.status === 'voided' ? 'destructive' : 'outline'}>
          {row.original.status === 'voided' ? '已作废' : '正常'}
        </Badge>
      ),
    },
    { accessorKey: 'secret', header: '隐藏列', meta: { card: 'hidden' } },
    {
      id: 'actions',
      header: '',
      cell: () => (
        <button type="button" onClick={onAction}>
          编辑
        </button>
      ),
    },
  ]
}

function mockViewport(mobile: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockReturnValue({
      matches: mobile,
      media: '(max-width: 767px)',
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }),
  )
}

function renderTable(props: Partial<Parameters<typeof DataTable<Row, unknown>>[0]> = {}) {
  return render(
    <DataTable
      columns={makeColumns()}
      data={rows}
      totalCount={rows.length}
      page={{ pageNumber: 1, pageSize: 20 }}
      onPageChange={() => undefined}
      {...props}
    />,
  )
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  window.localStorage.clear()
})

describe('DataTable 视图模式', () => {
  it('桌面默认表格视图', () => {
    mockViewport(false)
    renderTable()

    expect(screen.getByRole('table')).toBeInTheDocument()
    expect(document.querySelector('[data-slot="card-list"]')).not.toBeInTheDocument()
  })

  it('移动端默认卡片视图:标题/状态徽标/字段行,hidden 列不出现', () => {
    mockViewport(true)
    renderTable()

    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    const cardList = document.querySelector('[data-slot="card-list"]')
    expect(cardList).toBeInTheDocument()
    // 首列(姓名)为标题,直接显示值
    expect(screen.getByText('张三')).toBeInTheDocument()
    // status 列 Badge 在卡内
    expect(screen.getByText('已作废')).toBeInTheDocument()
    // 其余列以 标签+值 成对出现
    expect(screen.getAllByText('金额')).toHaveLength(2)
    expect(screen.getByText('100.00')).toBeInTheDocument()
    // meta.card === 'hidden' 列不渲染
    expect(screen.queryByText('隐藏列')).not.toBeInTheDocument()
    expect(screen.queryByText('x')).not.toBeInTheDocument()
  })

  it('切换按钮在两种视图间切换,有 tableId 时持久化', async () => {
    mockViewport(true)
    const { unmount } = renderTable({ tableId: 'test-table' })

    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: '切换为表格视图' }))
    expect(screen.getByRole('table')).toBeInTheDocument()
    expect(window.localStorage.getItem('atlas.table.test-table.view')).toBe('table')

    // 重挂载后沿用持久化的选择(即便移动端缺省是卡片)
    unmount()
    renderTable({ tableId: 'test-table' })
    expect(screen.getByRole('table')).toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: '切换为卡片视图' }))
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    expect(window.localStorage.getItem('atlas.table.test-table.view')).toBe('card')
  })

  it('卡片可点击触发 onRowClick,操作区不触发', async () => {
    mockViewport(true)
    const onRowClick = vi.fn()
    const onAction = vi.fn()
    render(
      <DataTable
        columns={makeColumns(onAction)}
        data={rows}
        totalCount={rows.length}
        page={{ pageNumber: 1, pageSize: 20 }}
        onPageChange={() => undefined}
        onRowClick={onRowClick}
      />,
    )

    await userEvent.click(screen.getByText('张三'))
    expect(onRowClick).toHaveBeenCalledWith(rows[0])

    onRowClick.mockClear()
    await userEvent.click(screen.getAllByRole('button', { name: '编辑' })[0]!)
    expect(onAction).toHaveBeenCalled()
    expect(onRowClick).not.toHaveBeenCalled()
  })

  it('移动端空态显示 emptyText', () => {
    mockViewport(true)
    renderTable({ data: [], totalCount: 0, emptyText: '没有数据' })

    expect(screen.getByText('没有数据')).toBeInTheDocument()
  })
})
