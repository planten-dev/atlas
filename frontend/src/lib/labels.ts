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

export const DEAL_STATUS_LABELS: Record<string, string> = {
  closed: '已成交',
  not_closed: '未成交',
}

export const CUSTOMER_TYPE_LABELS: Record<string, string> = {
  new: '新客',
  returning: '老客',
}

export const DEAL_TYPE_LABELS: Record<string, string> = {
  non_salon: '非院装',
  salon: '院装',
}

export const COLLABORATION_TYPE_LABELS: Record<string, string> = {
  expert_consultation: '专家诊',
  self_sale: '自销',
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
  sale_date: '成交日期',
  deal_status: '成交状态',
  customer_type: '客户类型',
  deal_type: '成交类型',
  content_category_id: '内容类型',
  handler_user_id: '处理人',
  paid_amount: '已收金额',
  unpaid_amount: '未收金额',
  collaboration_type: '协作类型',
  expert_user_id: '专家',
  expert_department_id: '专家部门',
  consultant_user_id: '咨询师',
  consultant_department_id: '咨询师部门',
  doctor_user_id: '医生',
  record_group_id: '批次',
  operation_total_count: '可操作次数',
  // 次数/耗用
  sales_record_id: '销售记录',
  total_count: '总次数',
  used_count: '已用次数',
  remaining_count: '剩余次数',
  operated_at: '操作时间',
  operator_user_id: '操作人',
  operation_count: '操作次数',
  // 用户/部门
  dingtalk_user_id: '钉钉用户ID',
  parent_id: '上级部门',
  source: '来源',
  external_id: '外部ID',
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
