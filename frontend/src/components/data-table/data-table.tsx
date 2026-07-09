import {
  flexRender,
  getCoreRowModel,
  useReactTable,
  type ColumnDef,
  type RowData,
} from '@tanstack/react-table'

declare module '@tanstack/react-table' {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  interface ColumnMeta<TData extends RowData, TValue> {
    /** 列设置面板显示名(表头为函数组件时必填)。 */
    title?: string
    /** 表头与单元格统一对齐方向(数字列用 right)。 */
    align?: 'left' | 'center' | 'right'
  }
}

const ALIGN_CLASS = {
  left: undefined,
  center: 'text-center',
  right: 'text-right',
} as const
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
  /** 提供后启用"列设置"(顺序/显隐,存 localStorage,键含该 id)。 */
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

/** 服务端分页 DataTable(设计 §3):数据由外部 hook 提供,组件只负责渲染与分页交互。 */
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

  return (
    <div className="flex flex-col gap-1.5">
      {tableId && (
        <div className="flex justify-end">
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
        </div>
      )}
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
      <DataTablePagination page={page} totalCount={totalCount} onPageChange={onPageChange} />
    </div>
  )
}
