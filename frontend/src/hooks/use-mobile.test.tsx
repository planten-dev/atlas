import { describe, expect, it, vi } from 'vitest'
import { renderHook } from '@testing-library/react'
import { useIsMobile, useMediaQuery } from '@/hooks/use-mobile'

function mockMatchMedia(matches: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockImplementation((query: string) => ({
      matches,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  )
}

describe('useMediaQuery / useIsMobile', () => {
  it('匹配时返回 true(移动视口)', () => {
    mockMatchMedia(true)
    const { result } = renderHook(() => useIsMobile())
    expect(result.current).toBe(true)
  })

  it('不匹配时返回 false(桌面视口)', () => {
    mockMatchMedia(false)
    const { result } = renderHook(() => useMediaQuery('(max-width: 767px)'))
    expect(result.current).toBe(false)
  })
})
