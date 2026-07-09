import { PickerBase } from '@/components/pickers/PickerBase'
import { useUserOptions } from '@/hooks/useUsers'

/** 人员选择器:全量 + 前端过滤(缺口 #3 接受),展示姓名+职位。 */
export function UserPicker({
  value,
  onChange,
  disabled,
  placeholder = '选择人员',
  className,
}: {
  value: string | undefined
  onChange: (value: string | undefined) => void
  disabled?: boolean
  placeholder?: string
  className?: string
}) {
  const { options, isLoading } = useUserOptions()
  return (
    <PickerBase
      items={options.map((u) => ({
        value: u.id,
        label: u.title ? `${u.name}(${u.title})` : u.name,
        keywords: [u.name],
      }))}
      value={value}
      onChange={onChange}
      disabled={disabled}
      isLoading={isLoading}
      placeholder={placeholder}
      searchPlaceholder="搜索人员…"
      className={className}
    />
  )
}
