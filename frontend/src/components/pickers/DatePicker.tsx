import { useState } from 'react'
import { isValid, parseISO } from 'date-fns'
import { zhCN } from 'date-fns/locale'
import { CalendarIcon, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Calendar } from '@/components/ui/calendar'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { toDateParam } from '@/lib/date'
import { cn } from '@/lib/utils'

interface DatePickerProps {
  /** yyyy-MM-dd */
  value: string | undefined
  onChange: (value: string | undefined) => void
  placeholder?: string
  disabled?: boolean
  clearable?: boolean
  id?: string
  className?: string
  'aria-label'?: string
  'aria-invalid'?: boolean
}

/** 日期选择:Popover + Calendar 替代原生 date input,值为 yyyy-MM-dd 字符串。 */
export function DatePicker({
  value,
  onChange,
  placeholder = '选择日期',
  disabled,
  clearable = true,
  id,
  className,
  ...aria
}: DatePickerProps) {
  const [open, setOpen] = useState(false)
  const parsed = value ? parseISO(value) : undefined
  const selected = parsed && isValid(parsed) ? parsed : undefined

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            id={id}
            variant="outline"
            disabled={disabled}
            className={cn('w-full justify-between font-normal', className)}
            {...aria}
          />
        }
      >
        <span
          className={cn(
            'flex min-w-0 items-center gap-2',
            !selected && 'text-muted-foreground',
          )}
        >
          <CalendarIcon className="size-4 shrink-0 opacity-50" />
          <span className="truncate">{selected ? value : placeholder}</span>
        </span>
        {clearable && selected && (
          // Button 对内部 svg 统一 pointer-events-none,清除区必须是可点击元素
          <span
            role="button"
            aria-label="清除日期"
            tabIndex={-1}
            className="pointer-events-auto rounded-sm p-0.5 opacity-50 hover:bg-muted hover:opacity-100"
            onClick={(e) => {
              e.stopPropagation()
              onChange(undefined)
            }}
          >
            <X className="pointer-events-none size-3.5 shrink-0" />
          </span>
        )}
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0" align="start">
        <Calendar
          mode="single"
          locale={zhCN}
          selected={selected}
          defaultMonth={selected}
          onSelect={(date) => {
            onChange(date ? toDateParam(date) : undefined)
            setOpen(false)
          }}
        />
      </PopoverContent>
    </Popover>
  )
}
