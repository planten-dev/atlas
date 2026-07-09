import { useEffect, useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { PickerBase } from '@/components/pickers/PickerBase'
import { CustomerFormFields, useCustomerForm } from '@/components/customers/CustomerForm'
import { customersListOptions, customerDetailOptions, useCreateCustomer } from '@/hooks/useCustomers'
import { notify } from '@/lib/notify'

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
          onCreated={(id) => {
            onChange(id)
            setCreateOpen(false)
          }}
        />
      )}
    </>
  )
}

function CreateCustomerDialog({
  open,
  onOpenChange,
  initialName,
  onCreated,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  initialName: string
  onCreated: (customerId: string) => void
}) {
  const form = useCustomerForm({ name: initialName })
  const createMutation = useCreateCustomer()

  const submit = form.handleSubmit((values) => {
    createMutation.mutate(
      {
        name: values.name,
        department_id: values.department_id,
        system_id: values.system_id,
        store_id: values.store_id,
        remark: values.remark || undefined,
        status: 'active',
      },
      {
        onSuccess: (outcome) => {
          if (outcome.kind === 'applied') {
            notify.success('客户已创建')
            onCreated(outcome.data.id)
          } else {
            notify.info('客户已提交审批,通过后可选择')
            onOpenChange(false)
          }
          form.reset()
        },
        onError: (error) => notify.error(error),
      },
    )
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>新建客户</DialogTitle>
        </DialogHeader>
        <form
          className="flex flex-col gap-4"
          onSubmit={(e) => {
            e.stopPropagation()
            void submit(e)
          }}
        >
          <CustomerFormFields form={form} />
          <Button type="submit" disabled={createMutation.isPending}>
            {createMutation.isPending ? '创建中…' : '创建并选择'}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}
