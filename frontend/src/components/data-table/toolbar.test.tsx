import { afterEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { DataTableToolbar } from './toolbar'

function mockBrowser({ mobile, userAgent }: { mobile: boolean; userAgent: string }) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockReturnValue({
      matches: mobile,
      media: '(max-width: 767px)',
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }),
  )
  vi.spyOn(window.navigator, 'userAgent', 'get').mockReturnValue(userAgent)
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('DataTableToolbar CSV export', () => {
  const exportConfig = {
    filename: 'test',
    columns: [{ header: '值', value: (row: { value: string }) => row.value }],
    rows: [{ value: '1' }],
  }

  it('钉钉移动端隐藏下载并提示到电脑端操作', () => {
    mockBrowser({ mobile: true, userAgent: 'Mozilla/5.0 AliApp(DingTalk/7.5.0)' })
    render(<DataTableToolbar exportConfig={exportConfig} />)

    expect(screen.queryByRole('button', { name: /导出 CSV/ })).not.toBeInTheDocument()
    expect(screen.getByText('请在电脑端导出 CSV')).toBeInTheDocument()
  })

  it('桌面浏览器继续显示导出按钮', () => {
    mockBrowser({
      mobile: false,
      userAgent: 'Mozilla/5.0 Chrome/126.0 Safari/537.36',
    })
    render(<DataTableToolbar exportConfig={exportConfig} />)

    expect(screen.getByRole('button', { name: /导出 CSV/ })).toBeInTheDocument()
  })
})

describe('DataTableToolbar 筛选折叠', () => {
  const desktopUa = 'Mozilla/5.0 Chrome/126.0 Safari/537.36'

  it('无筛选内容时不渲染筛选按钮', () => {
    mockBrowser({ mobile: false, userAgent: desktopUa })
    render(<DataTableToolbar />)

    expect(screen.queryByRole('button', { name: /筛选/ })).not.toBeInTheDocument()
  })

  it('桌面默认展开,点击可收起', async () => {
    mockBrowser({ mobile: false, userAgent: desktopUa })
    render(
      <DataTableToolbar>
        <input placeholder="按名称筛选" />
      </DataTableToolbar>,
    )

    expect(screen.getByPlaceholderText('按名称筛选')).toBeInTheDocument()
    const toggle = screen.getByRole('button', { name: /筛选/ })
    expect(toggle).toHaveAttribute('aria-expanded', 'true')

    await userEvent.click(toggle)
    expect(screen.queryByPlaceholderText('按名称筛选')).not.toBeInTheDocument()
    expect(toggle).toHaveAttribute('aria-expanded', 'false')
  })

  it('移动端默认收起,点击可展开', async () => {
    mockBrowser({ mobile: true, userAgent: desktopUa })
    render(
      <DataTableToolbar>
        <input placeholder="按名称筛选" />
      </DataTableToolbar>,
    )

    expect(screen.queryByPlaceholderText('按名称筛选')).not.toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: /筛选/ }))
    expect(screen.getByPlaceholderText('按名称筛选')).toBeInTheDocument()
  })
})
