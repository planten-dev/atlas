import { describe, expect, it } from 'vitest'
import type { CatalogEntryResponse } from '@/hooks/usePermissionsAdmin'
import { normalizePolicyAction, policyActionsForObject } from './policy-actions'

const catalog: CatalogEntryResponse[] = [
  {
    object: 'sales:records',
    actions: ['read', 'write'],
    group: '销售',
    label: '销售记录及可操作次数',
  },
  {
    object: 'sales:payments',
    actions: ['approve'],
    group: '审核',
    label: '销售付款',
  },
]

describe('policyActionsForObject', () => {
  it('付款权限只提供 approve 和对象通配动作', () => {
    expect(policyActionsForObject(catalog, 'sales:payments')).toEqual(['approve', '*'])
  })

  it('选择付款对象时把无效的默认 read 切换为 approve', () => {
    expect(normalizePolicyAction(catalog, 'sales:payments', 'read')).toBe('approve')
  })

  it('切换对象时保留仍然有效的动作', () => {
    expect(normalizePolicyAction(catalog, 'sales:records', 'write')).toBe('write')
  })

  it('销售记录保留目录中定义的读写动作', () => {
    expect(policyActionsForObject(catalog, 'sales:records')).toEqual(['read', 'write', '*'])
  })

  it('全局通配对象允许所有动作', () => {
    expect(policyActionsForObject(catalog, '*')).toEqual(['read', 'write', 'approve', '*'])
  })
})
