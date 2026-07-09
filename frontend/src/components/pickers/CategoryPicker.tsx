import { useQuery } from '@tanstack/react-query'
import { PickerBase } from '@/components/pickers/PickerBase'
import { activeCategoriesOptions } from '@/hooks/useCategories'

/** 内容类型(产品类别)选择器。 */
export function CategoryPicker({
  value,
  onChange,
  disabled,
  placeholder = '选择内容类型',
  className,
}: {
  value: string | undefined
  onChange: (value: string | undefined) => void
  disabled?: boolean
  placeholder?: string
  className?: string
}) {
  const { data: categories, isLoading } = useQuery(activeCategoriesOptions)
  return (
    <PickerBase
      items={(categories ?? []).map((c) => ({ value: c.id, label: c.category_name }))}
      value={value}
      onChange={onChange}
      disabled={disabled}
      isLoading={isLoading}
      placeholder={placeholder}
      searchPlaceholder="搜索类别…"
      className={className}
    />
  )
}
