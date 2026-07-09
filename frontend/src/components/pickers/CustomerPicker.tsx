import { useEffect, useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { PickerBase } from '@/components/pickers/PickerBase'
import { CreateCustomerDialog } from '@/components/customers/CreateCustomerDialog'
import { customersListOptions, customerDetailOptions } from '@/hooks/useCustomers'

function useDebounced<T>(value: T, delay = 300): T {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delay)
    return () => clearTimeout(timer)
  }, [value, delay])
  return debounced
}

/** 客户选择器:服务端 name_keyword 搜索(防抖)+ 内嵌新建客户 Dialog(设计 §9)。 */
export function CustomerPicker({
  value,
  onChange,
  disabled,
  allowCreate = true,
  className,
}: {
  value: string | undefined
  onChange: (value: string | undefined) => void
  disabled?: boolean
  allowCreate?: boolean
  className?: string
}) {
  const [keyword, setKeyword] = useState('')
  const debouncedKeyword = useDebounced(keyword)
  const [createOpen, setCreateOpen] = useState(false)

  const listQuery = useQuery(
    customersListOptions({
      status_filter: 'active',
      name_keyword: debouncedKeyword || undefined,
      page_size: 50,
    }),
  )
  // 已选客户可能不在当前搜索结果里,单独取 detail 以显示名称
  const selectedQuery = useQuery({
    ...customerDetailOptions(value ?? ''),
    enabled: Boolean(value),
  })

  const items = useMemo(() => {
    const base = (listQuery.data?.items ?? []).map((c) => ({ value: c.id, label: c.name }))
    if (value && selectedQuery.data && !base.some((i) => i.value === value)) {
      base.unshift({ value, label: selectedQuery.data.name })
    }
    return base
  }, [listQuery.data, selectedQuery.data, value])

  return (
    <>
      <PickerBase
        items={items}
        value={value}
        onChange={onChange}
        disabled={disabled}
        isLoading={listQuery.isLoading && !listQuery.data}
        placeholder="搜索并选择客户"
        searchPlaceholder="输入客户姓名搜索…"
        emptyText={debouncedKeyword ? '没有匹配的客户' : '输入关键字搜索客户'}
        onSearchChange={setKeyword}
        className={className}
        footer={
          allowCreate ? (
            <div className="border-t p-1">
              <Button
                variant="ghost"
                size="sm"
                className="w-full justify-start"
                onClick={() => setCreateOpen(true)}
              >
                <Plus />
                新建客户{debouncedKeyword ? `“${debouncedKeyword}”` : ''}
              </Button>
            </div>
          ) : undefined
        }
      />
      {allowCreate && (
        <CreateCustomerDialog
          open={createOpen}
          onOpenChange={setCreateOpen}
          initialName={keyword}
          submitText="创建并选择"
          onCreated={(id) => {
            onChange(id)
            setCreateOpen(false)
          }}
        />
      )}
    </>
  )
}
