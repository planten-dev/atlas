/** 后端错误码 → 中文文案;未映射的展示 server message。 */

const ERROR_MESSAGES: Record<string, string> = {
  permission_denied: '无权限,请联系管理员',
  unauthorized: '登录已失效,请重新登录',
  event_not_pending: '该事件已被处理,无法重复审批',
  duplicate_review: '你已审批过该事件',
  event_not_reviewable: '该事件不可审批',
  unknown_resource_type: '未知的资源类型',
  not_found: '记录不存在或已被删除',
  conflict: '数据冲突,请刷新后重试',
  validation_error: '提交内容校验未通过',
  bad_gateway: '钉钉服务暂不可用,请稍后重试',
  internal_error: '服务器内部错误,请稍后重试',
}

export function errorMessage(code: string, fallback: string): string {
  return ERROR_MESSAGES[code] ?? fallback
}
