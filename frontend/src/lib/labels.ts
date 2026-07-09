/** 线上枚举的中文标签。event_type / approval_status 是数字枚举(与后端一致)。 */

export const EVENT_TYPE_LABELS: Record<number, string> = {
  0: '新建',
  1: '更新',
  2: '删除',
  3: '通过',
  4: '驳回',
  5: '自定义',
}

export const APPROVAL_STATUS_LABELS: Record<number, string> = {
  0: '无需审批',
  1: '待审批',
  2: '已通过',
  3: '已驳回',
}

export const CUSTOMER_TYPE_LABELS: Record<string, string> = {
  new: '新客',
  returning: '老客',
}

export const DEAL_TYPE_LABELS: Record<string, string> = {
  non_salon: '非沙龙',
  salon: '沙龙',
}

export const RECORD_TYPE_LABELS: Record<string, string> = {
  sale: '销售',
  service: '服务',
}

export const PAYMENT_TYPE_LABELS: Record<string, string> = {
  initial: '首款',
  collection: '回款',
}

export const PERFORMANCE_STATUS_LABELS: Record<string, string> = {
  pending: '待入账',
  posted: '已入账',
}

export const RECORD_STATUS_LABELS: Record<string, string> = {
  active: '正常',
  voided: '已作废',
}

export const ENTITY_STATUS_LABELS: Record<string, string> = {
  active: '启用',
  disabled: '停用',
}

export const ROLE_KIND_LABELS: Record<string, string> = {
  position: '职位',
  department: '部门',
  custom: '自定义',
}

export const POLICY_EFFECT_LABELS: Record<string, string> = {
  allow: '允许',
  deny: '拒绝',
}

export const SUBJECT_KIND_LABELS: Record<string, string> = {
  user: '用户',
  role: '角色',
}

export const RESOURCE_TYPE_LABELS: Record<string, string> = {
  products: '产品',
  product_categories: '产品类别',
  customers: '客户',
  systems: '体系',
  stores: '门店',
  users: '用户',
  departments: '部门',
  sales_records: '销售记录',
  sales_record_operation_counts: '次数账户',
  sales_record_operation_usages: '耗用记录',
  events: '事件',
}

/** diff 视图 / 表单的字段中文名(跨域并集,按需补充)。 */
export const FIELD_LABELS: Record<string, string> = {
  id: 'ID',
  name: '名称',
  status: '状态',
  remark: '备注',
  created_at: '创建时间',
  updated_at: '更新时间',
  // 产品
  unit_price: '单价',
  category_id: '类别',
  requires_operation_count: '需要次数账户',
  // 客户
  phone: '电话',
  department_id: '部门',
  system_id: '体系',
  store_id: '门店',
  // 销售
  customer_id: '客户',
  record_date: '成交日期',
  record_type: '记录类型',
  customer_type: '客户类型',
  deal_type: '成交类型',
  handler_user_id: '处理人',
  receivable_amount: '应收金额',
  paid_amount: '已收金额',
  outstanding_amount: '未收金额',
  expert_user_id: '专家',
  consultant_user_id: '咨询师',
  doctor_user_id: '医生',
  product_id: '产品',
  item_name: '项目名称',
  operation_total_count: '可操作次数',
  // 付款
  payment_type: '付款类型',
  paid_at: '支付时间',
  performance_status: '业绩状态',
  guide_user_id: '导购',
  allocation_ratio: '分配比例',
  allocated_amount: '分配金额',
  // 次数/耗用
  sales_record_id: '销售记录',
  sales_record_line_id: '销售明细行',
  total_count: '总次数',
  used_count: '已用次数',
  remaining_count: '剩余次数',
  operated_at: '操作时间',
  operator_user_id: '操作人',
  operation_count: '操作次数',
  // 历史审计字段(旧销售模型 diff 渲染,勿删)
  sale_date: '成交日期',
  deal_status: '成交状态',
  content_category_id: '内容类型',
  unpaid_amount: '未收金额',
  collaboration_type: '协作类型',
  expert_department_id: '专家部门',
  consultant_department_id: '咨询师部门',
  record_group_id: '批次',
  // 用户/部门
  dingtalk_user_id: '钉钉用户ID',
  parent_id: '上级部门',
  source: '来源',
  external_id: '外部ID',
}

/**
 * 权限对象 → 展示分组/中文名。与后端 authz_catalog.rs 的 builtin 目录保持一致;
 * catalog 接口需要 system:permissions:read,普通用户拿不到,故本地静态维护。
 */
export const PERMISSION_OBJECT_META: Record<string, { group: string; label: string }> = {
  customers: { group: '销售', label: '客户' },
  'sales:records': { group: '销售', label: '销售记录' },
  'sales:operation-counts': { group: '销售', label: '次数账户' },
  'sales:operation-usages': { group: '销售', label: '耗用记录' },
  products: { group: '商品', label: '产品' },
  'products:categories': { group: '商品', label: '产品类别' },
  systems: { group: '门店', label: '门店体系' },
  stores: { group: '门店', label: '门店' },
  events: { group: '审核', label: '审核事件' },
  users: { group: '系统', label: '用户管理' },
  departments: { group: '系统', label: '部门' },
  'system:permissions': { group: '系统', label: '权限管理' },
}

export const PERMISSION_ACTION_LABELS: Record<string, string> = {
  read: '查看',
  write: '编辑',
  approve: '审批',
  '*': '全部',
}

export interface PermissionGroupItem {
  object: string
  label: string
  actions: string[]
}

export interface PermissionGroup {
  group: string
  items: PermissionGroupItem[]
}

const PERMISSION_GROUP_ORDER = ['销售', '商品', '门店', '审核', '系统', '其他']
const PERMISSION_ACTION_ORDER = ['read', 'write', 'approve']
const PERMISSION_OBJECT_ORDER = Object.keys(PERMISSION_OBJECT_META)

/**
 * 把后端已解析的 `object:action` 权限串按对象聚合、按目录分组,
 * 未收录的对象归入"其他"并原样展示(前端字典落后于后端时不静默丢失)。
 */
export function groupPermissions(permissions: Iterable<string>): PermissionGroup[] {
  const actionsByObject = new Map<string, Set<string>>()
  for (const perm of permissions) {
    const idx = perm.lastIndexOf(':')
    const object = idx > 0 ? perm.slice(0, idx) : perm
    const action = idx > 0 ? perm.slice(idx + 1) : ''
    const actions = actionsByObject.get(object) ?? new Set<string>()
    if (action) actions.add(action)
    actionsByObject.set(object, actions)
  }

  const itemsByGroup = new Map<string, PermissionGroupItem[]>()
  for (const [object, actions] of actionsByObject) {
    const meta = PERMISSION_OBJECT_META[object]
    const group = meta?.group ?? '其他'
    const items = itemsByGroup.get(group) ?? []
    items.push({
      object,
      label: meta?.label ?? object,
      actions: [...actions].sort(
        (a, b) => rankOf(PERMISSION_ACTION_ORDER, a) - rankOf(PERMISSION_ACTION_ORDER, b),
      ),
    })
    itemsByGroup.set(group, items)
  }

  return [...itemsByGroup.entries()]
    .sort(([a], [b]) => rankOf(PERMISSION_GROUP_ORDER, a) - rankOf(PERMISSION_GROUP_ORDER, b))
    .map(([group, items]) => ({
      group,
      items: items.sort(
        (a, b) =>
          rankOf(PERMISSION_OBJECT_ORDER, a.object) - rankOf(PERMISSION_OBJECT_ORDER, b.object) ||
          a.object.localeCompare(b.object),
      ),
    }))
}

function rankOf(order: readonly string[], value: string): number {
  const idx = order.indexOf(value)
  return idx === -1 ? order.length : idx
}

export function fieldLabel(key: string): string {
  return FIELD_LABELS[key] ?? key
}

export function resourceTypeLabel(resourceType: string): string {
  return RESOURCE_TYPE_LABELS[resourceType] ?? resourceType
}

export function eventTypeLabel(eventType: number): string {
  return EVENT_TYPE_LABELS[eventType] ?? `类型${eventType}`
}

export function approvalStatusLabel(status: number): string {
  return APPROVAL_STATUS_LABELS[status] ?? `状态${status}`
}
