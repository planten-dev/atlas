import { describe, expect, it } from 'vitest'
import { CATALOG, CATALOG_GROUPS, visibleCatalog } from '@/reports/registry'

/** 应用内真实路由前缀(available 条目的 route 必须能落在这些路径上)。 */
const VALID_ROUTE_PREFIXES = [
  '/sales',
  '/usages',
  '/counts',
  '/customers',
  '/approvals',
  '/products',
  '/performance',
  '/admin/audit',
]

describe('业务目录 registry 完整性(设计 §8)', () => {
  it('共 44 个有效条目', () => {
    expect(CATALOG).toHaveLength(44)
  })

  it('id 全局唯一', () => {
    const ids = CATALOG.map((item) => item.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('分组均为 6 个既定分组之一', () => {
    for (const item of CATALOG) {
      expect(CATALOG_GROUPS).toContain(item.group)
    }
  })

  it('分组条目数与旧平台目录一致', () => {
    const countByGroup = new Map<string, number>()
    for (const item of CATALOG) {
      countByGroup.set(item.group, (countByGroup.get(item.group) ?? 0) + 1)
    }
    expect(countByGroup.get('日报类')).toBe(7)
    expect(countByGroup.get('消耗查询类')).toBe(6)
    expect(countByGroup.get('事业部数据报表')).toBe(8)
    expect(countByGroup.get('数据报表')).toBe(7)
    expect(countByGroup.get('绩效导出表')).toBe(7)
    expect(countByGroup.get('辅助类')).toBe(9)
  })

  it('available 条目必有 route 且指向真实路由', () => {
    for (const item of CATALOG.filter((c) => c.status === 'available')) {
      expect(item.route, `${item.id} 缺 route`).toBeDefined()
      const path = item.route!.split('?')[0]!
      expect(
        VALID_ROUTE_PREFIXES.some((prefix) => path === prefix || path.startsWith(`${prefix}/`)),
        `${item.id} 的 route ${item.route} 不在已知路由内`,
      ).toBe(true)
    }
  })

  it('available 条目必有 perm(菜单权限过滤依据)', () => {
    for (const item of CATALOG.filter((c) => c.status === 'available')) {
      expect(item.perm, `${item.id} 缺 perm`).toBeDefined()
    }
  })

  it('reserved 条目必有 note(说明等待的后端能力)', () => {
    for (const item of CATALOG.filter((c) => c.status === 'reserved')) {
      expect(item.note, `${item.id} 缺 note`).toBeTruthy()
    }
  })

  it('沿用旧名的核心条目存在', () => {
    const titles = CATALOG.map((c) => c.title)
    expect(titles).toContain('销售日报')
    expect(titles).toContain('仪器手工及医疗消耗')
    expect(titles).toContain('内购日报')
    expect(titles).toContain('销售成交明细(私定)')
    expect(titles).toContain('客户应付金额')
  })
})

describe('visibleCatalog 权限过滤', () => {
  it('available 条目按权限过滤', () => {
    const visible = visibleCatalog(new Set(['sales:records:read']))
    expect(visible.some((i) => i.id === 'report-card-deals')).toBe(true)
    expect(visible.some((i) => i.id === 'daily-sales')).toBe(false) // 需要 write
  })

  it('reserved 条目对所有登录用户可见(显式预留)', () => {
    const visible = visibleCatalog(new Set())
    expect(visible.some((i) => i.id === 'daily-internal-purchase')).toBe(true)
  })
})
