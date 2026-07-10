/**
 * 业务目录注册表(设计 §8):旧平台 6 个分组、去重后 44 个有效条目。
 * - 沿用旧平台分类与名称(员工零学习成本);未实现条目显式预留(reserved)而非消失。
 * - 后端每落地一个统计接口 → 翻一条 status → 菜单自动点亮,无需改其他代码。
 * - 侧边栏"业务"区、命令面板、/reports/:id 占位页均由本文件驱动。
 *
 * 注:设计文档仅点名了部分条目;标 [占位] 的 reserved 条目名称需上线前与旧平台目录核对更名。
 * 类别预设(私定/医疗等)用类别名传参,运行时按名解析 id("私定"上线初始化建类别后自动归位)。
 */

export type CatalogGroup =
  | '日报类'
  | '消耗查询类'
  | '事业部数据报表'
  | '数据报表'
  | '绩效导出表'
  | '辅助类'

export interface CatalogItem {
  id: string
  /** 沿用旧平台名称 */
  title: string
  group: CatalogGroup
  status: 'available' | 'reserved'
  /** available:真实路由(可带预设参数) */
  route?: string
  /** 菜单按此权限过滤 */
  perm?: string
  /** reserved:等待的后端能力 */
  note?: string
}

export const CATALOG: CatalogItem[] = [
  /* ---------- 日报类(7) ---------- */
  { id: 'daily-sales', title: '销售日报', group: '日报类', status: 'available', route: '/sales/new', perm: 'sales:records:write' },
  { id: 'daily-usage-device-medical', title: '仪器手工及医疗消耗', group: '日报类', status: 'available', route: '/usages/new', perm: 'sales:operation-usages:write' },
  { id: 'daily-usage-private', title: '私定手工', group: '日报类', status: 'available', route: '/usages/new?category=私定', perm: 'sales:operation-usages:write' },
  { id: 'daily-usage-medical', title: '医疗手工', group: '日报类', status: 'available', route: '/usages/new?category=医疗', perm: 'sales:operation-usages:write' },
  { id: 'daily-internal-purchase', title: '内购日报', group: '日报类', status: 'reserved', note: '等待后端 deal_type 枚举扩展(内购)' },
  { id: 'daily-deposit', title: '收定金', group: '日报类', status: 'reserved', note: '等待定金域建设' },
  { id: 'daily-appointment', title: '预约跟进', group: '日报类', status: 'reserved', note: '等待预约域建设' },

  /* ---------- 消耗查询类(6) ---------- */
  { id: 'query-device-remaining', title: '仪器剩余', group: '消耗查询类', status: 'reserved', note: '等待 operation-counts/list 支持按类别筛选(缺口 #2),届时接 /counts 预设' },
  { id: 'query-card-remaining', title: '卡项未消耗', group: '消耗查询类', status: 'reserved', note: '等待 operation-counts/list 支持按类别筛选(缺口 #2)' },
  { id: 'query-private-remaining', title: '私定剩余', group: '消耗查询类', status: 'reserved', note: '等待 operation-counts/list 支持按类别筛选(缺口 #2)' },
  { id: 'query-usage-detail', title: '消耗明细查询', group: '消耗查询类', status: 'available', route: '/usages', perm: 'sales:operation-usages:read' },
  { id: 'query-guide-usage-rank', title: '美导消耗排名', group: '消耗查询类', status: 'reserved', note: '等待统计域建设' },
  { id: 'query-department-usage-rank', title: '部门消耗排名', group: '消耗查询类', status: 'reserved', note: '等待统计域建设' },

  /* ---------- 事业部数据报表(8,全部预留,统计域) ---------- */
  { id: 'bu-performance-rank', title: '事业部业绩排名 [占位]', group: '事业部数据报表', status: 'reserved', note: '等待统计域建设' },
  { id: 'bu-headcount', title: '事业部人头统计 [占位]', group: '事业部数据报表', status: 'reserved', note: '等待统计域建设' },
  { id: 'bu-share', title: '事业部业绩占比 [占位]', group: '事业部数据报表', status: 'reserved', note: '等待统计域建设' },
  { id: 'bu-summary', title: '事业部业绩总表 [占位]', group: '事业部数据报表', status: 'reserved', note: '等待统计域建设' },
  { id: 'bu-new-customer', title: '事业部新客统计 [占位]', group: '事业部数据报表', status: 'reserved', note: '等待统计域建设' },
  { id: 'bu-returning-customer', title: '事业部老客统计 [占位]', group: '事业部数据报表', status: 'reserved', note: '等待统计域建设' },
  { id: 'bu-store-rank', title: '事业部门店排名 [占位]', group: '事业部数据报表', status: 'reserved', note: '等待统计域建设' },
  { id: 'bu-monthly', title: '事业部月度总表 [占位]', group: '事业部数据报表', status: 'reserved', note: '等待统计域建设' },

  /* ---------- 数据报表(7) ---------- */
  { id: 'report-card-deals', title: '卡项成交明细', group: '数据报表', status: 'reserved', note: '类别/产品维度筛选等待新列表接口(销售明细行已改为产品维度)' },
  { id: 'report-device-deals', title: '仪器成交明细', group: '数据报表', status: 'reserved', note: '类别/产品维度筛选等待新列表接口(销售明细行已改为产品维度)' },
  { id: 'report-medical-deals', title: '医疗成交明细', group: '数据报表', status: 'reserved', note: '类别/产品维度筛选等待新列表接口(销售明细行已改为产品维度)' },
  { id: 'report-product-private-deals', title: '产品及私定成交明细', group: '数据报表', status: 'available', route: '/sales', perm: 'sales:records:read' },
  { id: 'report-product-sales-rank', title: '产品销售排名', group: '数据报表', status: 'reserved', note: '等待统计域建设' },
  { id: 'report-system-performance', title: '体系业绩排名', group: '数据报表', status: 'reserved', note: '等待统计域建设' },
  { id: 'report-card-rank', title: '卡项排名', group: '数据报表', status: 'reserved', note: '等待统计域建设' },

  /* ---------- 绩效导出表(7,全部预留;过渡用列表页 CSV 导出) ---------- */
  { id: 'perf-beautician', title: '美容师绩效导出 [占位]', group: '绩效导出表', status: 'reserved', note: '绩效口径未定型;过渡用销售/耗用列表 CSV 导出' },
  { id: 'perf-consultant', title: '咨询师绩效导出 [占位]', group: '绩效导出表', status: 'reserved', note: '绩效口径未定型;过渡用 CSV 导出' },
  { id: 'perf-doctor', title: '医生绩效导出 [占位]', group: '绩效导出表', status: 'reserved', note: '绩效口径未定型;过渡用 CSV 导出' },
  { id: 'perf-store-manager', title: '店长绩效导出 [占位]', group: '绩效导出表', status: 'reserved', note: '绩效口径未定型;过渡用 CSV 导出' },
  { id: 'perf-guide', title: '美导绩效导出 [占位]', group: '绩效导出表', status: 'reserved', note: '绩效口径未定型;过渡用 CSV 导出' },
  { id: 'perf-check', title: '员工提成核对表 [占位]', group: '绩效导出表', status: 'reserved', note: '等待核对域建设' },
  { id: 'perf-import', title: '绩效数据导入 [占位]', group: '绩效导出表', status: 'reserved', note: '等待导入域建设' },

  /* ---------- 辅助类(9;"姓名匹配"不迁移) ---------- */
  { id: 'aux-private-deals', title: '销售成交明细(私定)', group: '辅助类', status: 'reserved', note: '类别/产品维度筛选等待新列表接口(销售明细行已改为产品维度)' },
  { id: 'aux-private-device-remaining', title: '私定仪器剩余', group: '辅助类', status: 'reserved', note: '等待 operation-counts/list 支持按类别筛选(缺口 #2),届时接 /counts 预设' },
  { id: 'aux-customer-payable', title: '客户应付金额', group: '辅助类', status: 'reserved', note: '等待 sales-records/list 支持未收金额筛选(缺口 #4),届时接 /sales 预设' },
  { id: 'aux-customer-archive', title: '客户档案查询', group: '辅助类', status: 'available', route: '/customers', perm: 'customers:read' },
  { id: 'aux-remaining-query', title: '剩余次数查询', group: '辅助类', status: 'available', route: '/counts', perm: 'sales:records:read' },
  { id: 'aux-sales-summary', title: '销售汇总', group: '辅助类', status: 'reserved', note: '等待统计域建设' },
  { id: 'aux-staff-performance', title: '人员业绩统计', group: '辅助类', status: 'available', route: '/performance', perm: 'sales:performance:read' },
  { id: 'aux-allocation-ratio', title: '分配比例', group: '辅助类', status: 'reserved', note: '等待统计域建设' },
  { id: 'aux-audit-query', title: '操作记录查询', group: '辅助类', status: 'available', route: '/admin/audit', perm: 'events:read' },
]

export const CATALOG_GROUPS: CatalogGroup[] = [
  '日报类',
  '消耗查询类',
  '事业部数据报表',
  '数据报表',
  '绩效导出表',
  '辅助类',
]

export function catalogById(id: string): CatalogItem | undefined {
  return CATALOG.find((item) => item.id === id)
}

/** 菜单/命令面板投影:reserved 条目对所有登录用户可见(显式预留);available 条目按权限过滤。 */
export function visibleCatalog(permissions: ReadonlySet<string>): CatalogItem[] {
  return CATALOG.filter(
    (item) => item.status === 'reserved' || !item.perm || permissions.has(item.perm),
  )
}
