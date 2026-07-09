import { fieldLabel } from '@/lib/labels'
import { formatAmount, isValidAmount } from '@/lib/money'
import { cn } from '@/lib/utils'

/**
 * old/new 字段级 diff(设计 §9):
 * - 取两对象键并集,逐字段一行,只高亮变化。
 * - create(old=null)呈现"新增";delete(new=null)呈现"删除"。
 * - 桌面左右两列、移动上下堆叠;不甩原始 JSON。
 */

const HIDDEN_KEYS = new Set(['id', 'created_at', 'updated_at'])
const MONEY_KEYS = new Set(['paid_amount', 'unpaid_amount', 'unit_price'])

function formatValue(key: string, value: unknown): string {
  if (value === null || value === undefined || value === '') return '—'
  if (typeof value === 'boolean') return value ? '是' : '否'
  if (MONEY_KEYS.has(key) && typeof value === 'string' && isValidAmount(value)) {
    return formatAmount(value)
  }
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    return value as Record<string, unknown>
  }
  return null
}

export interface DiffRow {
  key: string
  label: string
  oldText: string
  newText: string
  changed: boolean
}

/** 导出供测试:由 old/new 计算 diff 行。 */
export function computeDiffRows(oldValue: unknown, newValue: unknown): DiffRow[] {
  const oldRecord = asRecord(oldValue)
  const newRecord = asRecord(newValue)
  const keys = [...new Set([...Object.keys(oldRecord ?? {}), ...Object.keys(newRecord ?? {})])]
    .filter((k) => !HIDDEN_KEYS.has(k))

  return keys.map((key) => {
    const oldText = oldRecord ? formatValue(key, oldRecord[key]) : '—'
    const newText = newRecord ? formatValue(key, newRecord[key]) : '—'
    return {
      key,
      label: fieldLabel(key),
      oldText,
      newText,
      changed: oldText !== newText,
    }
  })
}

export function DiffView({
  oldValue,
  newValue,
}: {
  oldValue: unknown
  newValue: unknown
}) {
  const rows = computeDiffRows(oldValue, newValue)
  const isCreate = asRecord(oldValue) === null && asRecord(newValue) !== null
  const isDelete = asRecord(oldValue) !== null && asRecord(newValue) === null

  if (rows.length === 0) {
    return <p className="text-sm text-muted-foreground">无字段变更内容</p>
  }

  return (
    <div className="flex flex-col gap-1">
      <div className="hidden grid-cols-[8rem_1fr_1fr] gap-2 border-b pb-1 text-xs text-muted-foreground md:grid">
        <span>字段</span>
        <span>{isCreate ? '' : '变更前'}</span>
        <span>{isDelete ? '' : '变更后'}</span>
      </div>
      {rows.map((row) => (
        <div
          key={row.key}
          className={cn(
            'grid grid-cols-1 gap-1 rounded-md px-2 py-1.5 md:grid-cols-[8rem_1fr_1fr] md:gap-2',
            row.changed && 'bg-amber-50 dark:bg-amber-950/30',
          )}
        >
          <span className="text-sm font-medium text-muted-foreground">{row.label}</span>
          {!isCreate && (
            <span
              className={cn(
                'break-all text-sm',
                row.changed && !isDelete && 'text-destructive line-through',
              )}
            >
              <span className="mr-1 text-xs text-muted-foreground md:hidden">前:</span>
              {row.oldText}
            </span>
          )}
          {!isDelete && (
            <span className={cn('break-all text-sm', row.changed && 'font-medium text-green-700 dark:text-green-400')}>
              <span className="mr-1 text-xs text-muted-foreground md:hidden">后:</span>
              {row.newText}
            </span>
          )}
        </div>
      ))}
    </div>
  )
}
