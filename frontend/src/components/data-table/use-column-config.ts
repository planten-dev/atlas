import { useCallback, useState } from 'react'

/** 列配置:显示顺序 + 隐藏列。仅存 localStorage,不做同步。 */
export interface ColumnConfig {
  order: string[]
  hidden: string[]
}

const storageKey = (tableId: string) => `atlas.table.${tableId}.columns`

export function loadColumnConfig(tableId: string): ColumnConfig | null {
  try {
    const raw = localStorage.getItem(storageKey(tableId))
    if (!raw) return null
    const parsed = JSON.parse(raw) as ColumnConfig
    if (!Array.isArray(parsed.order) || !Array.isArray(parsed.hidden)) return null
    return parsed
  } catch {
    return null
  }
}

/**
 * 与当前列集合对账:代码里列增删后,存档中已删除的列丢弃、
 * 新增的列按自然顺序追加到末尾,隐藏列只保留仍存在的。
 */
export function reconcileColumnConfig(
  columnIds: readonly string[],
  saved: ColumnConfig | null,
): ColumnConfig {
  if (!saved) return { order: [...columnIds], hidden: [] }
  const known = new Set(columnIds)
  const order = saved.order.filter((id) => known.has(id))
  for (const id of columnIds) {
    if (!order.includes(id)) order.push(id)
  }
  return { order, hidden: saved.hidden.filter((id) => known.has(id)) }
}

export function useColumnConfig(tableId: string | undefined, columnIds: readonly string[]) {
  const [config, setConfig] = useState<ColumnConfig>(() =>
    reconcileColumnConfig(columnIds, tableId ? loadColumnConfig(tableId) : null),
  )

  const apply = useCallback(
    (next: ColumnConfig) => {
      setConfig(next)
      if (!tableId) return
      try {
        localStorage.setItem(storageKey(tableId), JSON.stringify(next))
      } catch {
        // 隐私模式等写入失败:仅本次会话生效
      }
    },
    [tableId],
  )

  const toggleColumn = useCallback(
    (id: string) => {
      apply({
        ...config,
        hidden: config.hidden.includes(id)
          ? config.hidden.filter((h) => h !== id)
          : [...config.hidden, id],
      })
    },
    [apply, config],
  )

  const moveColumn = useCallback(
    (id: string, delta: -1 | 1) => {
      const index = config.order.indexOf(id)
      const target = index + delta
      if (index < 0 || target < 0 || target >= config.order.length) return
      const order = [...config.order]
      order.splice(index, 1)
      order.splice(target, 0, id)
      apply({ ...config, order })
    },
    [apply, config],
  )

  const resetConfig = useCallback(() => {
    setConfig({ order: [...columnIds], hidden: [] })
    if (!tableId) return
    try {
      localStorage.removeItem(storageKey(tableId))
    } catch {
      // ignore
    }
  }, [tableId, columnIds])

  const isCustomized =
    config.hidden.length > 0 || config.order.some((id, i) => id !== columnIds[i])

  return { config, toggleColumn, moveColumn, resetConfig, isCustomized }
}
