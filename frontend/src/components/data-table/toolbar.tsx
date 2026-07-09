import type { ReactNode } from 'react'
import { Download } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { buildCsv, downloadCsv, type CsvColumn } from '@/lib/csv'
import { notify } from '@/lib/notify'

interface DataTableToolbarProps<T> {
  /** 筛选控件槽位 */
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
  return (
    <div className="flex flex-wrap items-center gap-2">
      <div className="flex flex-1 flex-wrap items-center gap-2">{children}</div>
      <div className="flex items-center gap-2">
        {exportConfig && (
          <Button
            variant="outline"
            size="sm"
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
  )
}
