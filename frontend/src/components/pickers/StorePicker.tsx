import { useQuery } from '@tanstack/react-query'
import { PickerBase } from '@/components/pickers/PickerBase'
import { storeOptionsForPicker } from '@/hooks/useStores'

/** 门店选择器:级联于体系(父未选禁用)。 */
export function StorePicker({
  systemId,
  value,
  onChange,
  disabled,
  placeholder,
  className,
}: {
  systemId: string | undefined
  value: string | undefined
  onChange: (value: string | undefined) => void
  disabled?: boolean
  placeholder?: string
  className?: string
}) {
  const { data: stores, isLoading } = useQuery(storeOptionsForPicker(systemId))
  return (
    <PickerBase
      items={(stores ?? []).map((s) => ({ value: s.id, label: s.name }))}
      value={value}
      onChange={onChange}
      disabled={disabled || !systemId}
      isLoading={isLoading}
      placeholder={placeholder ?? (systemId ? '选择门店' : '请先选择体系')}
      searchPlaceholder="搜索门店…"
      className={className}
    />
  )
}
