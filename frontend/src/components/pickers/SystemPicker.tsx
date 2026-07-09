import { useQuery } from '@tanstack/react-query'
import { PickerBase } from '@/components/pickers/PickerBase'
import { systemOptionsForPicker } from '@/hooks/useSystems'

/** 体系选择器:级联于部门(父未选禁用)。 */
export function SystemPicker({
  departmentId,
  value,
  onChange,
  disabled,
  placeholder,
  className,
}: {
  departmentId: string | undefined
  value: string | undefined
  onChange: (value: string | undefined) => void
  disabled?: boolean
  placeholder?: string
  className?: string
}) {
  const { data: systems, isLoading } = useQuery(systemOptionsForPicker(departmentId))
  return (
    <PickerBase
      items={(systems ?? []).map((s) => ({ value: s.id, label: s.name }))}
      value={value}
      onChange={onChange}
      disabled={disabled || !departmentId}
      isLoading={isLoading}
      placeholder={placeholder ?? (departmentId ? '选择体系' : '请先选择部门')}
      searchPlaceholder="搜索体系…"
      className={className}
    />
  )
}
