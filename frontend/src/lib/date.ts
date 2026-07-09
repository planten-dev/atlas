import { format, parseISO } from 'date-fns'

/** ISO 日期/时间 → `yyyy-MM-dd`;空值返回 '-'。 */
export function formatDate(value: string | null | undefined): string {
  if (!value) return '-'
  try {
    return format(parseISO(value), 'yyyy-MM-dd')
  } catch {
    return value
  }
}

/** ISO 时间 → `yyyy-MM-dd HH:mm`;空值返回 '-'。 */
export function formatDateTime(value: string | null | undefined): string {
  if (!value) return '-'
  try {
    return format(parseISO(value), 'yyyy-MM-dd HH:mm')
  } catch {
    return value
  }
}

/** Date → 后端 `date` 格式(yyyy-MM-dd)。 */
export function toDateParam(date: Date): string {
  return format(date, 'yyyy-MM-dd')
}

/** Date → datetime-local 输入值(本地时区,分钟精度)。 */
export function toLocalDateTimeInput(date: Date): string {
  return format(date, "yyyy-MM-dd'T'HH:mm")
}
