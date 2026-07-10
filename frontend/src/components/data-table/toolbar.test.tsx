import { afterEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
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
