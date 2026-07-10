import { afterEach, describe, expect, it, vi } from 'vitest'
import { renderHook } from '@testing-library/react'
import { useIsMobile, useMediaQuery } from '@/hooks/use-mobile'

function mockMatchMedia(matches: boolean) {
  const addEventListener = vi.fn()
  const removeEventListener = vi.fn()
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockImplementation((query: string) => ({
      matches,
      media: query,
      addEventListener,
      removeEventListener,
    })),
  )
  return { addEventListener, removeEventListener }
}

afterEach(() => vi.unstubAllGlobals())

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

  it('现代 MediaQueryList 使用 addEventListener 并正确清理', () => {
    const listeners = mockMatchMedia(true)
    const { unmount } = renderHook(() => useIsMobile())

    expect(listeners.addEventListener).toHaveBeenCalledWith('change', expect.any(Function))
    unmount()
    expect(listeners.removeEventListener).toHaveBeenCalledWith('change', expect.any(Function))
  })

  it('旧 WebView 使用 addListener/removeListener', () => {
    const addListener = vi.fn()
    const removeListener = vi.fn()
    vi.stubGlobal(
      'matchMedia',
      vi.fn().mockReturnValue({
        matches: true,
        media: '(max-width: 767px)',
        addListener,
        removeListener,
      }),
    )

    const { unmount } = renderHook(() => useIsMobile())
    expect(addListener).toHaveBeenCalledWith(expect.any(Function))
    unmount()
    expect(removeListener).toHaveBeenCalledWith(expect.any(Function))
  })
})
