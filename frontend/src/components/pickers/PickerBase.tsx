import { useState, type ReactNode } from 'react'
import { Check, ChevronsUpDown, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'

export interface PickerItem {
  value: string
  label: string
  /** 额外搜索词(如拼音、职位) */
  keywords?: string[]
  /** 树形缩进层级 */
  depth?: number
  disabled?: boolean
}

interface PickerBaseProps {
  items: PickerItem[]
  value: string | undefined
  onChange: (value: string | undefined) => void
  placeholder?: string
  searchPlaceholder?: string
  emptyText?: string
  disabled?: boolean
  isLoading?: boolean
  clearable?: boolean
  /** 服务端搜索:输入变化回调(否则 cmdk 本地过滤) */
  onSearchChange?: (keyword: string) => void
  footer?: ReactNode
  className?: string
}

/** 业务选择器公共骨架:Popover + Command(设计 §3 components/pickers)。 */
export function PickerBase({
  items,
  value,
  onChange,
  placeholder = '请选择',
  searchPlaceholder = '搜索…',
  emptyText = '无匹配结果',
  disabled,
  isLoading,
  clearable = true,
  onSearchChange,
  footer,
  className,
}: PickerBaseProps) {
  const [open, setOpen] = useState(false)
  const selected = items.find((item) => item.value === value)

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            variant="outline"
            role="combobox"
            aria-expanded={open}
            disabled={disabled}
            className={cn('w-full justify-between font-normal', className)}
          />
        }
      >
        <span className={cn('truncate', !selected && 'text-muted-foreground')}>
          {isLoading ? '加载中…' : (selected?.label ?? placeholder)}
        </span>
        <span className="flex items-center gap-1">
          {clearable && selected && (
            // Button 对内部 svg 统一 pointer-events-none,清除区必须是可点击元素
            <span
              role="button"
              aria-label="清除选择"
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
          <ChevronsUpDown className="size-4 shrink-0 opacity-50" />
        </span>
      </PopoverTrigger>
      <PopoverContent className="w-(--anchor-width) p-0" align="start">
        <Command shouldFilter={!onSearchChange}>
          <CommandInput
            placeholder={searchPlaceholder}
            onValueChange={onSearchChange}
          />
          <CommandList>
            <CommandEmpty>{emptyText}</CommandEmpty>
            <CommandGroup>
              {items.map((item) => (
                <CommandItem
                  key={item.value}
                  value={item.value}
                  keywords={[item.label, ...(item.keywords ?? [])]}
                  disabled={item.disabled}
                  onSelect={() => {
                    onChange(item.value)
                    setOpen(false)
                  }}
                >
                  <span
                    className="flex-1 truncate"
                    style={{ paddingLeft: `${(item.depth ?? 0) * 0.75}rem` }}
                  >
                    {item.label}
                  </span>
                  <Check
                    className={cn('size-4', value === item.value ? 'opacity-100' : 'opacity-0')}
                  />
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
          {footer}
        </Command>
      </PopoverContent>
    </Popover>
  )
}
