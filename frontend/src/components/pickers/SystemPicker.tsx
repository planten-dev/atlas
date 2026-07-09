import { useQuery } from '@tanstack/react-query'
import { PickerBase } from '@/components/pickers/PickerBase'
import { systemOptionsForPicker } from '@/hooks/useSystems'

/** 体系选择器(体系为顶层归属,不再挂部门)。 */
export function SystemPicker({
  value,
  onChange,
  disabled,
  placeholder = '选择体系',
  className,
}: {
  value: string | undefined
  onChange: (value: string | undefined) => void
  disabled?: boolean
  placeholder?: string
  className?: string
}) {
  const { data: systems, isLoading } = useQuery(systemOptionsForPicker)
  return (
    <PickerBase
      items={(systems ?? []).map((s) => ({ value: s.id, label: s.name }))}
      value={value}
      onChange={onChange}
      disabled={disabled}
      isLoading={isLoading}
      placeholder={placeholder}
      searchPlaceholder="搜索体系…"
      className={className}
    />
  )
}
