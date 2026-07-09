import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { CustomerFormFields, useCustomerForm } from '@/components/customers/CustomerForm'
import { requirePerm } from '@/auth/route-guard'
import { useCreateCustomer } from '@/hooks/useCustomers'
import { notify } from '@/lib/notify'

export const Route = createFileRoute('/_app/customers/new')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'customers:write'),
  component: CustomerNewPage,
})

function CustomerNewPage() {
  const navigate = useNavigate()
  const form = useCustomerForm()
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
            void navigate({ to: '/customers/$customerId', params: { customerId: outcome.data.id } })
          } else {
            notify.info('已提交审批,通过后生效')
            void navigate({ to: '/customers', search: {} })
          }
        },
        onError: (error) => notify.error(error),
      },
    )
  })

  return (
    <form className="mx-auto flex max-w-xl flex-col gap-4" onSubmit={submit}>
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon-sm" render={<Link to="/customers" search={{}} aria-label="返回" />}>
          <ArrowLeft />
        </Button>
        <h1 className="text-xl font-semibold">新建客户</h1>
      </div>
      <CustomerFormFields form={form} />
      <Button type="submit" size="lg" disabled={createMutation.isPending}>
        {createMutation.isPending ? '创建中…' : '创建'}
      </Button>
    </form>
  )
}
