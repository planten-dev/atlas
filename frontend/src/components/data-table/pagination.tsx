import { ChevronLeft, ChevronRight } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { PageState } from '@/components/data-table/data-table'

const PAGE_SIZES = [20, 50, 100, 200]

export function DataTablePagination({
  page,
  totalCount,
  onPageChange,
}: {
  page: PageState
  totalCount: number
  onPageChange: (page: PageState) => void
}) {
  const pageCount = Math.max(1, Math.ceil(totalCount / page.pageSize))

  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <p className="text-sm text-muted-foreground">共 {totalCount} 条</p>
      <div className="flex items-center gap-2">
        <Select
          items={PAGE_SIZES.map((size) => ({ value: String(size), label: `${size} 条/页` }))}
          value={String(page.pageSize)}
          onValueChange={(value) => {
            if (value) onPageChange({ pageNumber: 1, pageSize: Number(value) })
          }}
        >
          <SelectTrigger size="sm" className="w-28">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {PAGE_SIZES.map((size) => (
              <SelectItem key={size} value={String(size)}>
                {size} 条/页
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <div className="flex items-center gap-1">
          <Button
            variant="outline"
            size="icon-sm"
            disabled={page.pageNumber <= 1}
            onClick={() => onPageChange({ ...page, pageNumber: page.pageNumber - 1 })}
            aria-label="上一页"
          >
            <ChevronLeft />
          </Button>
          <span className="min-w-16 text-center text-sm">
            {page.pageNumber} / {pageCount}
          </span>
          <Button
            variant="outline"
            size="icon-sm"
            disabled={page.pageNumber >= pageCount}
            onClick={() => onPageChange({ ...page, pageNumber: page.pageNumber + 1 })}
            aria-label="下一页"
          >
            <ChevronRight />
          </Button>
        </div>
      </div>
    </div>
  )
}
