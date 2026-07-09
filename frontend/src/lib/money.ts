/**
 * 金额工具:后端 DECIMAL(12,2) 在 JSON 中为字符串。
 * 全部运算基于 BigInt 分(cents),禁止 parseFloat 参与合计。
 */

const AMOUNT_PATTERN = /^\d{1,10}(\.\d{1,2})?$/

export function isValidAmount(value: string): boolean {
  return AMOUNT_PATTERN.test(value)
}

/** 金额字符串 → BigInt 分。非法输入抛错。 */
export function toCents(value: string): bigint {
  if (!isValidAmount(value)) {
    throw new Error(`非法金额: ${value}`)
  }
  const [int = '0', frac = ''] = value.split('.')
  const fracPadded = frac.padEnd(2, '0')
  return BigInt(int) * 100n + BigInt(fracPadded)
}

/** BigInt 分 → 恒两位小数的金额字符串。 */
export function fromCents(cents: bigint): string {
  const negative = cents < 0n
  const abs = negative ? -cents : cents
  const int = abs / 100n
  const frac = (abs % 100n).toString().padStart(2, '0')
  return `${negative ? '-' : ''}${int}.${frac}`
}

/** 归一为两位小数展示(非法输入原样返回)。 */
export function formatAmount(value: string): string {
  if (!isValidAmount(value)) return value
  return fromCents(toCents(value))
}

export function addAmounts(a: string, b: string): string {
  return fromCents(toCents(a) + toCents(b))
}

/** 合计;跳过非法/空值以免整表崩溃(列表可能包含脏数据)。 */
export function sumAmounts(values: readonly string[]): string {
  let total = 0n
  for (const v of values) {
    if (isValidAmount(v)) total += toCents(v)
  }
  return fromCents(total)
}

/** a<b 返回 -1,a===b 返回 0,a>b 返回 1。 */
export function compareAmounts(a: string, b: string): -1 | 0 | 1 {
  const ca = toCents(a)
  const cb = toCents(b)
  if (ca < cb) return -1
  if (ca > cb) return 1
  return 0
}
