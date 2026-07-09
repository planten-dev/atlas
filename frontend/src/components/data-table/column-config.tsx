import { ChevronDown, ChevronUp, RotateCcw, Settings2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'

export interface ColumnConfigEntry {
  id: string
  label: string
  visible: boolean
  canHide: boolean
}

/** 列设置弹层:勾选显隐 + 上下移动排序 + 重置默认。 */
export function ColumnConfigButton({
  entries,
  onToggle,
  onMove,
  onReset,
  isCustomized,
}: {
  entries: ColumnConfigEntry[]
  onToggle: (id: string) => void
  onMove: (id: string, delta: -1 | 1) => void
  onReset: () => void
  isCustomized: boolean
}) {
  return (
    <Popover>
      <PopoverTrigger render={<Button variant="ghost" size="xs" />}>
        <Settings2 />
        列设置
      </PopoverTrigger>
      <PopoverContent align="end" className="w-60 p-2">
        <div className="flex flex-col gap-0.5">
          {entries.map((entry, index) => (
            <div key={entry.id} className="flex items-center gap-2 rounded-md px-1 py-0.5 hover:bg-muted/50">
              <Checkbox
                id={`col-${entry.id}`}
                checked={entry.visible}
                disabled={!entry.canHide}
                onCheckedChange={() => onToggle(entry.id)}
              />
              <label htmlFor={`col-${entry.id}`} className="flex-1 cursor-pointer truncate text-sm">
                {entry.label}
              </label>
              <Button
                variant="ghost"
                size="icon-xs"
                disabled={index === 0}
                onClick={() => onMove(entry.id, -1)}
                aria-label={`上移 ${entry.label}`}
              >
                <ChevronUp />
              </Button>
              <Button
                variant="ghost"
                size="icon-xs"
                disabled={index === entries.length - 1}
                onClick={() => onMove(entry.id, 1)}
                aria-label={`下移 ${entry.label}`}
              >
                <ChevronDown />
              </Button>
            </div>
          ))}
        </div>
        {isCustomized && (
          <Button variant="ghost" size="xs" className="mt-2 w-full" onClick={onReset}>
            <RotateCcw />
            重置默认
          </Button>
        )}
      </PopoverContent>
    </Popover>
  )
}
