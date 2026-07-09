import { useState } from 'react'
import { format, isValid, parseISO } from 'date-fns'
import { zhCN } from 'date-fns/locale'
import { CalendarIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Calendar } from '@/components/ui/calendar'
import { Input } from '@/components/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'

interface DateTimePickerProps {
  /** yyyy-MM-dd'T'HH:mm(本地时间,与 datetime-local 同构) */
  value: string | undefined
  onChange: (value: string) => void
  disabled?: boolean
  id?: string
  className?: string
  'aria-invalid'?: boolean
}

/** 日期时间选择:Calendar 选日期 + 时间输入,值保持 datetime-local 字符串格式。 */
export function DateTimePicker({
  value,
  onChange,
  disabled,
  id,
  className,
  ...aria
}: DateTimePickerProps) {
  const [open, setOpen] = useState(false)
  const parsed = value ? parseISO(value) : undefined
  const selected = parsed && isValid(parsed) ? parsed : undefined
  const timePart = value?.includes('T') ? value.slice(11, 16) : '00:00'

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            id={id}
            variant="outline"
            disabled={disabled}
            className={cn('w-full justify-start gap-2 font-normal', className)}
            {...aria}
          />
        }
      >
        <CalendarIcon className="size-4 shrink-0 opacity-50" />
        <span className={cn('truncate', !selected && 'text-muted-foreground')}>
          {selected ? format(selected, 'yyyy-MM-dd HH:mm') : '选择日期时间'}
        </span>
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0" align="start">
        <Calendar
          mode="single"
          locale={zhCN}
          selected={selected}
          defaultMonth={selected}
          onSelect={(date) => {
            if (date) onChange(`${format(date, 'yyyy-MM-dd')}T${timePart}`)
          }}
        />
        <div className="flex items-center gap-2 border-t p-3">
          <span className="text-sm text-muted-foreground">时间</span>
          <Input
            type="time"
            className="h-8"
            value={timePart}
            onChange={(e) => {
              const datePart = value?.slice(0, 10) ?? format(new Date(), 'yyyy-MM-dd')
              onChange(`${datePart}T${e.target.value || '00:00'}`)
            }}
            aria-label="时间"
          />
        </div>
      </PopoverContent>
    </Popover>
  )
}
