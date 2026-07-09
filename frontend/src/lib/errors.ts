/** 后端错误码 → 中文文案;未映射的展示 server message。 */

const ERROR_MESSAGES: Record<string, string> = {
  permission_denied: '无权限,请联系管理员',
  unauthorized: '登录已失效,请重新登录',
  event_not_pending: '该事件已被处理,无法重复审批',
  duplicate_review: '你已审批过该事件',
  event_not_reviewable: '该事件不可审批',
  unknown_resource_type: '未知的资源类型',
  cannot_disable_self: '不能停用自己的账号',
  protected_system_role: '内置系统角色受保护,无法执行该操作',
  role_not_found: '角色不存在或已被删除',
  user_not_found: '用户不存在或已被删除',
  // 销售域
  payment_exceeds_outstanding: '回款金额超过未收金额',
  collection_requires_sale_record: '服务记录不可登记回款',
  operation_count_requires_sale_record: '仅销售记录可操作次数账户',
  sales_record_voided: '销售记录已作废',
  sales_record_line_voided: '明细行已作废',
  operation_count_voided: '次数账户已作废',
  operation_usage_voided: '耗用记录已作废',
  operation_count_insufficient: '剩余次数不足',
  operation_count_below_used: '总次数不能低于已用次数',
  sales_record_has_active_operation_usages: '存在有效耗用记录,请先作废耗用再作废记录',
  sales_record_not_found: '销售记录不存在或已被删除',
  sales_record_line_not_found: '明细行不存在或已被删除',
  sales_payment_not_found: '付款记录不存在或已被删除',
  operation_count_not_found: '次数账户不存在',
  operation_usage_not_found: '耗用记录不存在或已被删除',
  product_not_found: '产品不存在或已被删除',
  customer_disabled: '客户已停用',
  product_disabled: '产品已停用',
  product_category_disabled: '产品类别已停用',
  store_system_mismatch: '门店与体系不匹配',
  product_has_references: '产品已被销售记录引用,无法删除,可改为停用',
  not_found: '记录不存在或已被删除',
  conflict: '数据冲突,请刷新后重试',
  validation_error: '提交内容校验未通过',
  bad_gateway: '钉钉服务暂不可用,请稍后重试',
  internal_error: '服务器内部错误,请稍后重试',
}

export function errorMessage(code: string, fallback: string): string {
  return ERROR_MESSAGES[code] ?? fallback
}
