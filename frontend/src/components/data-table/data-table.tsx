import { useState } from 'react'
import {
  flexRender,
  getCoreRowModel,
  useReactTable,
  type Cell,
  type ColumnDef,
  type RowData,
  type Table as TableInstance,
} from '@tanstack/react-table'
import { LayoutGrid, Table2 } from 'lucide-react'

declare module '@tanstack/react-table' {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  interface ColumnMeta<TData extends RowData, TValue> {
    /** 列设置面板显示名(表头为函数组件时必填)。 */
    title?: string
    /** 表头与单元格统一对齐方向(数字列用 right)。 */
    align?: 'left' | 'center' | 'right'
    /** 卡片视图中该列的角色:title=标题行 status=右上角徽标 hidden=不显示;缺省为普通字段行。 */
    card?: 'title' | 'status' | 'hidden'
  }
}

const ALIGN_CLASS = {
  left: undefined,
  center: 'text-center',
  right: 'text-right',
} as const
import { Button } from '@/components/ui/button'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Skeleton } from '@/components/ui/skeleton'
import { DataTablePagination } from '@/components/data-table/pagination'
import { ColumnConfigButton } from '@/components/data-table/column-config'
import { useColumnConfig } from '@/components/data-table/use-column-config'
import { useIsMobile } from '@/hooks/use-mobile'
import { cn } from '@/lib/utils'

export interface PageState {
  pageNumber: number
  pageSize: number
}

interface DataTableProps<TData, TValue> {
  columns: ColumnDef<TData, TValue>[]
  data: TData[]
  totalCount: number
  page: PageState
  onPageChange: (page: PageState) => void
  isLoading?: boolean
  emptyText?: string
  onRowClick?: (row: TData) => void
  rowClassName?: (row: TData) => string | undefined
  /** 提供后启用"列设置"(顺序/显隐,存 localStorage,键含该 id)与视图选择持久化。 */
  tableId?: string
}

function columnId<TData, TValue>(col: ColumnDef<TData, TValue>): string {
  return col.id ?? (col as { accessorKey?: string }).accessorKey ?? ''
}

function columnLabel<TData, TValue>(col: ColumnDef<TData, TValue>): string {
  if (typeof col.header === 'string' && col.header) return col.header
  const meta = col.meta as { title?: string } | undefined
  if (meta?.title) return meta.title
  const id = columnId(col)
  return id === 'actions' ? '操作' : id
}

type ViewMode = 'table' | 'card'

function viewStorageKey(tableId: string): string {
  return `atlas.table.${tableId}.view`
}

function loadViewMode(tableId: string | undefined, fallback: ViewMode): ViewMode {
  if (!tableId) return fallback
  try {
    const saved = window.localStorage.getItem(viewStorageKey(tableId))
    if (saved === 'table' || saved === 'card') return saved
  } catch {
    // 隐私模式等 storage 不可用:用缺省
  }
  return fallback
}

/** 服务端分页 DataTable(设计 §3):数据由外部 hook 提供,组件只负责渲染与分页交互。
 * 移动端默认卡片视图、桌面默认表格,可手动切换(有 tableId 时选择持久化)。 */
export function DataTable<TData, TValue>({
  columns,
  data,
  totalCount,
  page,
  onPageChange,
  isLoading = false,
  emptyText = '暂无数据',
  onRowClick,
  rowClassName,
  tableId,
}: DataTableProps<TData, TValue>) {
  const isMobile = useIsMobile()
  // 初始形态按当时视口决定,之后只随用户手动切换(不随视口变化重置)
  const [viewMode, setViewMode] = useState<ViewMode>(() =>
    loadViewMode(tableId, isMobile ? 'card' : 'table'),
  )
  const columnIds = columns.map(columnId).filter(Boolean)
  const { config, toggleColumn, moveColumn, resetConfig, isCustomized } = useColumnConfig(
    tableId,
    columnIds,
  )

  const byId = new Map(columns.map((col) => [columnId(col), col]))
  const orderedColumns = config.order
    .map((id) => byId.get(id))
    .filter((col): col is ColumnDef<TData, TValue> => col !== undefined)
  const visibleColumns = orderedColumns.filter((col) => !config.hidden.includes(columnId(col)))

  const table = useReactTable({
    data,
    columns: visibleColumns,
    getCoreRowModel: getCoreRowModel(),
    manualPagination: true,
    rowCount: totalCount,
  })

  const switchView = (mode: ViewMode) => {
    setViewMode(mode)
    if (tableId) {
      try {
        window.localStorage.setItem(viewStorageKey(tableId), mode)
      } catch {
        // storage 不可用时仅本次会话生效
      }
    }
  }

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-end gap-1">
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={viewMode === 'card' ? '切换为表格视图' : '切换为卡片视图'}
          onClick={() => switchView(viewMode === 'card' ? 'table' : 'card')}
        >
          {viewMode === 'card' ? <Table2 /> : <LayoutGrid />}
        </Button>
        {tableId && (
          <ColumnConfigButton
            entries={orderedColumns.map((col) => {
              const id = columnId(col)
              return {
                id,
                label: columnLabel(col),
                visible: !config.hidden.includes(id),
                // 至少保留一列可见
                canHide: config.hidden.includes(id) || visibleColumns.length > 1,
              }
            })}
            onToggle={toggleColumn}
            onMove={moveColumn}
            onReset={resetConfig}
            isCustomized={isCustomized}
          />
        )}
      </div>
      {viewMode === 'card' ? (
        <CardList
          table={table}
          isLoading={isLoading}
          emptyText={emptyText}
          onRowClick={onRowClick}
          rowClassName={rowClassName}
        />
      ) : (
        <div className="overflow-x-auto rounded-md border">
          <Table>
            <TableHeader>
              {table.getHeaderGroups().map((headerGroup) => (
                <TableRow key={headerGroup.id} className="bg-muted/50 hover:bg-muted/50">
                  {headerGroup.headers.map((header) => (
                    <TableHead
                      key={header.id}
                      className={cn('px-3', ALIGN_CLASS[header.column.columnDef.meta?.align ?? 'left'])}
                    >
                      {header.isPlaceholder
                        ? null
                        : flexRender(header.column.columnDef.header, header.getContext())}
                    </TableHead>
                  ))}
                </TableRow>
              ))}
            </TableHeader>
            <TableBody>
              {isLoading ? (
                Array.from({ length: 5 }).map((_, i) => (
                  <TableRow key={i}>
                    {visibleColumns.map((_, j) => (
                      <TableCell key={j} className="px-3">
                        <Skeleton className="h-4 w-full" />
                      </TableCell>
                    ))}
                  </TableRow>
                ))
              ) : table.getRowModel().rows.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={visibleColumns.length}
                    className="h-24 text-center text-muted-foreground"
                  >
                    {emptyText}
                  </TableCell>
                </TableRow>
              ) : (
                table.getRowModel().rows.map((row) => (
                  <TableRow
                    key={row.id}
                    className={cn(
                      onRowClick && 'cursor-pointer',
                      rowClassName?.(row.original),
                    )}
                    onClick={onRowClick ? () => onRowClick(row.original) : undefined}
                  >
                    {row.getVisibleCells().map((cell) => (
                      <TableCell
                        key={cell.id}
                        className={cn(
                          'px-3',
                          ALIGN_CLASS[cell.column.columnDef.meta?.align ?? 'left'],
                        )}
                      >
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </TableCell>
                    ))}
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      )}
      <DataTablePagination page={page} totalCount={totalCount} onPageChange={onPageChange} />
    </div>
  )
}

/* ---------------- 卡片视图 ---------------- */

interface CardCells<TData, TValue> {
  title?: Cell<TData, TValue>
  status?: Cell<TData, TValue>
  actions?: Cell<TData, TValue>
  fields: Cell<TData, TValue>[]
}

/** 按 meta.card 角色分拣可见单元格;缺省约定:第一个非 status/actions 列为标题,id 为 status/actions 的列各归其位。 */
function splitCardCells<TData, TValue>(cells: Cell<TData, TValue>[]): CardCells<TData, TValue> {
  const result: CardCells<TData, TValue> = { fields: [] }
  const rest: Cell<TData, TValue>[] = []

  for (const cell of cells) {
    const role = cell.column.columnDef.meta?.card
    if (role === 'hidden') continue
    if (role === 'title' && !result.title) {
      result.title = cell
      continue
    }
    if (role === 'status' && !result.status) {
      result.status = cell
      continue
    }
    if (!role && cell.column.id === 'status' && !result.status) {
      result.status = cell
      continue
    }
    if (!role && cell.column.id === 'actions' && !result.actions) {
      result.actions = cell
      continue
    }
    rest.push(cell)
  }

  if (!result.title && rest.length > 0) {
    result.title = rest.shift()
  }
  result.fields = rest
  return result
}

function CardList<TData, TValue>({
  table,
  isLoading,
  emptyText,
  onRowClick,
  rowClassName,
}: {
  table: TableInstance<TData>
  isLoading: boolean
  emptyText: string
  onRowClick?: (row: TData) => void
  rowClassName?: (row: TData) => string | undefined
}) {
  if (isLoading) {
    return (
      <div className="flex flex-col gap-2" data-slot="card-list">
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="flex flex-col gap-2 rounded-md border p-3">
            <Skeleton className="h-5 w-2/3" />
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-full" />
          </div>
        ))}
      </div>
    )
  }

  const rows = table.getRowModel().rows
  if (rows.length === 0) {
    return (
      <div className="rounded-md border py-16 text-center text-sm text-muted-foreground">
        {emptyText}
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-2" data-slot="card-list">
      {rows.map((row) => {
        const { title, status, actions, fields } = splitCardCells(
          row.getVisibleCells() as Cell<TData, TValue>[],
        )
        return (
          <div
            key={row.id}
            className={cn(
              'flex flex-col gap-1.5 rounded-md border p-3',
              onRowClick && 'cursor-pointer transition-colors hover:bg-accent/50',
              rowClassName?.(row.original),
            )}
            onClick={onRowClick ? () => onRowClick(row.original) : undefined}
          >
            {(title || status) && (
              <div className="flex items-start justify-between gap-2">
                <span className="text-sm font-medium">
                  {title && flexRender(title.column.columnDef.cell, title.getContext())}
                </span>
                {status && flexRender(status.column.columnDef.cell, status.getContext())}
              </div>
            )}
            {fields.map((cell) => (
              <div key={cell.id} className="flex items-start justify-between gap-4 text-sm">
                <span className="shrink-0 text-muted-foreground">
                  {columnLabel(cell.column.columnDef)}
                </span>
                <span className="min-w-0 text-right">
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </span>
              </div>
            ))}
            {actions && (
              // 阻断冒泡:操作按钮不触发整卡 onRowClick
              <div className="flex justify-end" onClick={(e) => e.stopPropagation()}>
                {flexRender(actions.column.columnDef.cell, actions.getContext())}
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}
