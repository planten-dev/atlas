import { useState, type ReactNode } from 'react'
import { ChevronDown, Download, ListFilter } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { buildCsv, downloadCsv, type CsvColumn } from '@/lib/csv'
import { cn } from '@/lib/utils'
import { notify } from '@/lib/notify'
import { useIsMobile } from '@/hooks/use-mobile'
import { isDingTalkWebview } from '@/auth/dingtalk'

interface DataTableToolbarProps<T> {
  /** 筛选控件槽位(可折叠;移动端默认收起,桌面默认展开) */
  children?: ReactNode
  /** 提供导出配置则显示"导出 CSV(当前筛选)"按钮(设计 §8 过渡能力) */
  exportConfig?: {
    filename: string
    columns: CsvColumn<T>[]
    rows: readonly T[]
  }
  actions?: ReactNode
}

export function DataTableToolbar<T>({ children, exportConfig, actions }: DataTableToolbarProps<T>) {
  const isMobile = useIsMobile()
  const csvUnavailable = isMobile && isDingTalkWebview()
  const hasFilters = children !== undefined && children !== null
  // 移动端筛选默认收起(占屏太多),桌面默认展开;仅跟随初始形态,不随窗口变化重置
  const [filtersOpen, setFiltersOpen] = useState(() => !isMobile)

  return (
    <div className="flex flex-col gap-2">
      {/* 筛选区可能折成多行,动作按钮固定与第一行对齐(items-start + 同高按钮) */}
      <div className="flex flex-wrap items-start gap-2">
        <div className="flex flex-1 flex-wrap items-center gap-2">
          {hasFilters && (
            <Button
              variant="outline"
              aria-expanded={filtersOpen}
              onClick={() => setFiltersOpen((open) => !open)}
            >
              <ListFilter />
              筛选
              <ChevronDown className={cn('transition-transform', filtersOpen && 'rotate-180')} />
            </Button>
          )}
        </div>
        <div className="flex items-center gap-2">
          {exportConfig && csvUnavailable && (
            <span className="text-xs text-muted-foreground">请在电脑端导出 CSV</span>
          )}
          {exportConfig && !csvUnavailable && (
            <Button
              variant="outline"
              onClick={() => {
                if (exportConfig.rows.length === 0) {
                  notify.info('当前筛选无数据可导出')
                  return
                }
                downloadCsv(exportConfig.filename, buildCsv(exportConfig.columns, exportConfig.rows))
              }}
            >
              <Download />
              导出 CSV
            </Button>
          )}
          {actions}
        </div>
      </div>
      {hasFilters && filtersOpen && (
        <div className="flex flex-wrap items-center gap-2">{children}</div>
      )}
    </div>
  )
}
