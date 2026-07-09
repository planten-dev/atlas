/**
 * 审批流预留(设计 §5):每个域的 mutation 统一返回该联合类型。
 * 目前 ApplierRegistry 为空,写接口直改(恒 applied);
 * 将来某域接入审批后只改对应 hook,页面代码零改动。
 */
export type MutationOutcome<T> =
  | { kind: 'applied'; data: T }
  | { kind: 'submitted'; eventId: string }

export function applied<T>(data: T): MutationOutcome<T> {
  return { kind: 'applied', data }
}

/** mutation 成功后的统一提示文案。 */
export function outcomeMessage(outcome: MutationOutcome<unknown>, appliedText: string): string {
  return outcome.kind === 'applied' ? appliedText : '已提交审批,等待审批通过后生效'
}
