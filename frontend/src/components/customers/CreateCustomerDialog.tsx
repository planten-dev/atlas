import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { CustomerFormFields, useCustomerForm } from '@/components/customers/CustomerForm'
import { useCreateCustomer } from '@/hooks/useCustomers'
import { notify } from '@/lib/notify'

/** 新建客户弹窗:客户列表页与客户选择器内嵌新建共用。 */
export function CreateCustomerDialog({
  open,
  onOpenChange,
  initialName = '',
  submitText = '创建',
  onCreated,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  initialName?: string
  submitText?: string
  onCreated: (customerId: string) => void
}) {
  const form = useCustomerForm({ name: initialName })
  const createMutation = useCreateCustomer()

  const submit = form.handleSubmit((values) => {
    createMutation.mutate(
      {
        name: values.name,
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
            notify.info('客户已提交审批,通过后生效')
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
      {/* 外壳固定圆角,内容区单独滚动,滚动条不压边框/关闭按钮 */}
      <DialogContent className="grid max-h-[85vh] grid-rows-[auto_1fr] gap-4">
        <DialogHeader>
          <DialogTitle>新建客户</DialogTitle>
        </DialogHeader>
        <form
          className="-mx-2 flex flex-col gap-4 overflow-y-auto px-2 pb-1"
          onSubmit={(e) => {
            e.stopPropagation()
            void submit(e)
          }}
        >
          <CustomerFormFields form={form} />
          <Button type="submit" disabled={createMutation.isPending}>
            {createMutation.isPending ? '创建中…' : submitText}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}
