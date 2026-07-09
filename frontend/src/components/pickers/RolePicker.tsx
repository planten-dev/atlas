import { useQuery } from '@tanstack/react-query'
import { PickerBase } from '@/components/pickers/PickerBase'
import { rolesListOptions } from '@/hooks/usePermissionsAdmin'

/** 角色选择器:全量角色,按名称/编码搜索(权限面板·主体授权用)。 */
export function RolePicker({
  value,
  onChange,
  disabled,
  placeholder = '选择角色',
  className,
}: {
  value: string | undefined
  onChange: (value: string | undefined) => void
  disabled?: boolean
  placeholder?: string
  className?: string
}) {
  const { data: roles, isLoading } = useQuery(rolesListOptions)
  return (
    <PickerBase
      items={(roles ?? []).map((role) => ({
        value: role.id,
        label: `${role.name}(${role.code})`,
        keywords: [role.name, role.code],
      }))}
      value={value}
      onChange={onChange}
      disabled={disabled}
      isLoading={isLoading}
      placeholder={placeholder}
      searchPlaceholder="搜索角色…"
      className={className}
    />
  )
}
