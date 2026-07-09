import { useQuery } from '@tanstack/react-query'
import { PickerBase, type PickerItem } from '@/components/pickers/PickerBase'
import { departmentTreeOptions, type DepartmentNode } from '@/hooks/useDepartments'

function flattenTree(nodes: DepartmentNode[], depth = 0): PickerItem[] {
  return nodes.flatMap((node) => [
    { value: node.id, label: node.name, depth },
    ...flattenTree(node.children, depth + 1),
  ])
}

/** 部门选择器:全量拉取 + 前端建树,树形缩进展示(设计 §9)。 */
export function DepartmentPicker({
  value,
  onChange,
  disabled,
  placeholder = '选择部门',
  className,
}: {
  value: string | undefined
  onChange: (value: string | undefined) => void
  disabled?: boolean
  placeholder?: string
  className?: string
}) {
  const { data: tree, isLoading } = useQuery(departmentTreeOptions('active'))
  return (
    <PickerBase
      items={tree ? flattenTree(tree) : []}
      value={value}
      onChange={onChange}
      disabled={disabled}
      isLoading={isLoading}
      placeholder={placeholder}
      searchPlaceholder="搜索部门…"
      className={className}
    />
  )
}
