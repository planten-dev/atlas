import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useSuspenseQuery } from '@tanstack/react-query'
import { ArrowLeft } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { CustomerFormFields, useCustomerForm } from '@/components/customers/CustomerForm'
import { requirePerm } from '@/auth/route-guard'
import { customerDetailOptions, useUpdateCustomer } from '@/hooks/useCustomers'
import { notify } from '@/lib/notify'
import { outcomeMessage } from '@/hooks/mutation-result'

export const Route = createFileRoute('/_app/customers/$customerId/edit')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'customers:write'),
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(customerDetailOptions(params.customerId)),
  component: CustomerEditPage,
})

function CustomerEditPage() {
  const { customerId } = Route.useParams()
  const navigate = useNavigate()
  const { data: customer } = useSuspenseQuery(customerDetailOptions(customerId))

  const form = useCustomerForm({
    name: customer.name,
    department_id: customer.department_id,
    system_id: customer.system_id,
    store_id: customer.store_id,
    remark: customer.remark ?? '',
  })
  const updateMutation = useUpdateCustomer()

  const submit = form.handleSubmit((values) => {
    updateMutation.mutate(
      {
        customerId,
        body: {
          name: values.name,
          department_id: values.department_id,
          system_id: values.system_id,
          store_id: values.store_id,
          remark: values.remark || null,
        },
      },
      {
        onSuccess: (outcome) => {
          notify.success(outcomeMessage(outcome, '客户资料已更新'))
          void navigate({ to: '/customers/$customerId', params: { customerId } })
        },
        onError: (error) => notify.error(error),
      },
    )
  })

  return (
    <form className="mx-auto flex max-w-xl flex-col gap-4" onSubmit={submit}>
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="icon-sm"
          render={<Link to="/customers/$customerId" params={{ customerId }} aria-label="返回" />}
        >
          <ArrowLeft />
        </Button>
        <h1 className="text-xl font-semibold">编辑客户</h1>
      </div>
      <CustomerFormFields form={form} />
      <Button type="submit" size="lg" disabled={updateMutation.isPending}>
        {updateMutation.isPending ? '保存中…' : '保存'}
      </Button>
    </form>
  )
}
