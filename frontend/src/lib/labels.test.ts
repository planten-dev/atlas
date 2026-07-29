import { describe, expect, it } from 'vitest'
import { groupPermissions, PERMISSION_OBJECT_META } from './labels'

describe('groupPermissions', () => {
  it('按对象聚合动作并按目录分组', () => {
    const groups = groupPermissions([
      'sales:records:read',
      'sales:records:write',
      'customers:read',
      'products:read',
    ])
    expect(groups.map((g) => g.group)).toEqual(['销售', '商品'])
    const sales = groups[0]!
    expect(sales.items.map((i) => i.object)).toEqual(['customers', 'sales:records'])
    expect(sales.items[1]!.label).toBe('销售记录')
    expect(sales.items[1]!.actions).toEqual(['read', 'write'])
  })

  it('动作按 read → write → approve 排序', () => {
    const groups = groupPermissions(['products:approve', 'products:write', 'products:read'])
    expect(groups[0]!.items[0]!.actions).toEqual(['read', 'write', 'approve'])
  })

  it('多段对象取最后一段为动作', () => {
    const groups = groupPermissions(['sales:records:approve', 'system:permissions:read'])
    const objects = groups.flatMap((g) => g.items.map((i) => i.object))
    expect(objects).toContain('sales:records')
    expect(objects).toContain('system:permissions')
  })

  it('未收录对象归入"其他"且原样展示,不静默丢失', () => {
    const groups = groupPermissions(['finance:docs:approve'])
    expect(groups).toHaveLength(1)
    expect(groups[0]!.group).toBe('其他')
    expect(groups[0]!.items[0]!).toMatchObject({
      object: 'finance:docs',
      label: 'finance:docs',
      actions: ['approve'],
    })
  })

  it('分组顺序固定:销售 → 商品 → 门店 → 审核 → 系统 → 其他', () => {
    const groups = groupPermissions([
      'unknown:read',
      'users:read',
      'events:read',
      'stores:read',
      'products:read',
      'customers:read',
    ])
    expect(groups.map((g) => g.group)).toEqual(['销售', '商品', '门店', '审核', '系统', '其他'])
  })

  it('空集合返回空数组', () => {
    expect(groupPermissions([])).toEqual([])
  })

  it('目录字典覆盖后端运行时的全部 12 个对象', () => {
    expect(Object.keys(PERMISSION_OBJECT_META)).toHaveLength(12)
    expect(PERMISSION_OBJECT_META['sales:performance']).toEqual({ group: '销售', label: '人员业绩' })
    expect(PERMISSION_OBJECT_META).not.toHaveProperty('sales:operation-counts')
  })
})
