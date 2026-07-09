import { beforeEach, describe, expect, it } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import {
  loadColumnConfig,
  reconcileColumnConfig,
  useColumnConfig,
} from '@/components/data-table/use-column-config'

const COLUMNS = ['a', 'b', 'c']

beforeEach(() => {
  localStorage.clear()
})

describe('reconcileColumnConfig', () => {
  it('无存档时返回自然顺序、无隐藏', () => {
    expect(reconcileColumnConfig(COLUMNS, null)).toEqual({ order: ['a', 'b', 'c'], hidden: [] })
  })

  it('丢弃已删除的列,新列追加到末尾', () => {
    const saved = { order: ['c', 'removed', 'a'], hidden: ['removed', 'b'] }
    expect(reconcileColumnConfig(COLUMNS, saved)).toEqual({
      order: ['c', 'a', 'b'],
      hidden: ['b'],
    })
  })
})

describe('useColumnConfig', () => {
  it('toggle 隐藏/恢复列并持久化', () => {
    const { result } = renderHook(() => useColumnConfig('t1', COLUMNS))
    act(() => result.current.toggleColumn('b'))
    expect(result.current.config.hidden).toEqual(['b'])
    expect(loadColumnConfig('t1')?.hidden).toEqual(['b'])

    act(() => result.current.toggleColumn('b'))
    expect(result.current.config.hidden).toEqual([])
  })

  it('move 上移/下移,越界忽略', () => {
    const { result } = renderHook(() => useColumnConfig('t2', COLUMNS))
    act(() => result.current.moveColumn('c', -1))
    expect(result.current.config.order).toEqual(['a', 'c', 'b'])

    act(() => result.current.moveColumn('a', -1)) // 已在顶部
    expect(result.current.config.order).toEqual(['a', 'c', 'b'])

    act(() => result.current.moveColumn('b', 1)) // 已在底部
    expect(result.current.config.order).toEqual(['a', 'c', 'b'])
    expect(loadColumnConfig('t2')?.order).toEqual(['a', 'c', 'b'])
  })

  it('reset 恢复默认并清除存档', () => {
    const { result } = renderHook(() => useColumnConfig('t3', COLUMNS))
    act(() => {
      result.current.toggleColumn('a')
      result.current.moveColumn('c', -1)
    })
    expect(result.current.isCustomized).toBe(true)

    act(() => result.current.resetConfig())
    expect(result.current.config).toEqual({ order: ['a', 'b', 'c'], hidden: [] })
    expect(result.current.isCustomized).toBe(false)
    expect(loadColumnConfig('t3')).toBeNull()
  })

  it('从存档恢复初始状态', () => {
    localStorage.setItem(
      'atlas.table.t4.columns',
      JSON.stringify({ order: ['b', 'a', 'c'], hidden: ['c'] }),
    )
    const { result } = renderHook(() => useColumnConfig('t4', COLUMNS))
    expect(result.current.config).toEqual({ order: ['b', 'a', 'c'], hidden: ['c'] })
    expect(result.current.isCustomized).toBe(true)
  })

  it('存档损坏时回退默认', () => {
    localStorage.setItem('atlas.table.t5.columns', '{not-json')
    const { result } = renderHook(() => useColumnConfig('t5', COLUMNS))
    expect(result.current.config).toEqual({ order: ['a', 'b', 'c'], hidden: [] })
  })

  it('无 tableId 时仅内存生效,不写 localStorage', () => {
    const { result } = renderHook(() => useColumnConfig(undefined, COLUMNS))
    act(() => result.current.toggleColumn('a'))
    expect(result.current.config.hidden).toEqual(['a'])
    expect(localStorage.length).toBe(0)
  })
})
