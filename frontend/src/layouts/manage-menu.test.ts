import { describe, expect, it } from 'vitest'
import { MANAGE_ENTRIES, MANAGE_PERMS, visibleManageEntries } from './manage-menu'

describe('visibleManageEntries', () => {
  it('空权限集 → 空列表', () => {
    expect(visibleManageEntries(new Set())).toEqual([])
  })

  it('单权限只显示对应条目', () => {
    expect(visibleManageEntries(new Set(['systems:read'])).map((e) => e.to)).toEqual([
      '/org/systems',
    ])
    expect(visibleManageEntries(new Set(['customers:read'])).map((e) => e.to)).toEqual([
      '/customers',
    ])
    expect(visibleManageEntries(new Set(['stores:read'])).map((e) => e.to)).toEqual([
      '/org/stores',
    ])
  })

  it('全权限 → 3 条,顺序 客户/体系/门店', () => {
    const all = visibleManageEntries(
      new Set(['customers:read', 'systems:read', 'stores:read']),
    )
    expect(all.map((e) => e.title)).toEqual(['客户', '体系管理', '门店管理'])
  })

  it('无关权限不解锁条目', () => {
    expect(visibleManageEntries(new Set(['sales:records:read', 'customers:write']))).toEqual([])
  })
})

describe('MANAGE_PERMS', () => {
  it('与条目 perm 一致(防漂移)', () => {
    expect(MANAGE_PERMS).toEqual(MANAGE_ENTRIES.map((e) => e.perm))
  })
})
