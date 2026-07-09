import { useMemo } from 'react'
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router'
import { useQuery, useSuspenseQuery } from '@tanstack/react-query'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { ArrowLeft } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { requirePerm } from '@/auth/route-guard'
import { activeCategoriesOptions } from '@/hooks/useCategories'
import { salesDetailOptions, useUpdateSalesRecord } from '@/hooks/useSales'
import { notify } from '@/lib/notify'
import { outcomeMessage } from '@/hooks/mutation-result'
import {
  assembleRecords,
  buildSalesFormSchema,
  type SalesFormValues,
} from '../-form/schema'
import { CollaborationSection, CustomerSection, LinesSection } from '../-form/sections'

export const Route = createFileRoute('/_app/sales/$salesId/edit')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:records:write'),
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(salesDetailOptions(params.salesId)),
  component: SalesEditPage,
})

function SalesEditPage() {
  const { salesId } = Route.useParams()
  const navigate = useNavigate()
  const { data: record } = useSuspenseQuery(salesDetailOptions(salesId))
  const { data: categories } = useQuery(activeCategoriesOptions)
  const requiresMap = useMemo(
    () => new Map((categories ?? []).map((c) => [c.id, c.requires_operation_count])),
    [categories],
  )

  const form = useForm<SalesFormValues>({
    resolver: zodResolver(buildSalesFormSchema(requiresMap)),
    defaultValues: {
      customer_id: record.customer_id,
      sale_date: record.sale_date,
      system_id: record.system_id,
      store_id: record.store_id,
      customer_type: record.customer_type,
      deal_type: record.deal_type,
      deal_status: record.deal_status,
      collaboration_type: record.collaboration_type,
      expert_user_id: record.expert_user_id ?? '',
      consultant_user_id: record.consultant_user_id ?? '',
      doctor_user_id: record.doctor_user_id ?? '',
      handler_user_id: record.handler_user_id,
      lines: [
        {
          content_category_id: record.content_category_id,
          paid_amount: record.paid_amount,
          unpaid_amount: record.unpaid_amount,
          operation_total_count: record.operation_count?.total_count,
        },
      ],
    },
    mode: 'onBlur',
  })

  const updateMutation = useUpdateSalesRecord()

  const submit = form.handleSubmit((values) => {
    const [assembled] = assembleRecords(values, requiresMap)
    if (!assembled) return
    // update 契约无 operation_total_count(次数走专门接口调整)
    const { operation_total_count, ...body } = assembled
    void operation_total_count
    updateMutation.mutate(
      { salesRecordId: salesId, body },
      {
        onSuccess: (outcome) => {
          notify.success(outcomeMessage(outcome, '销售记录已更新'))
          void navigate({ to: '/sales/$salesId', params: { salesId } })
        },
        onError: (error) => notify.error(error),
      },
    )
  })

  return (
    <form className="mx-auto flex max-w-4xl flex-col gap-4" onSubmit={submit}>
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="icon-sm"
          render={<Link to="/sales/$salesId" params={{ salesId }} aria-label="返回" />}
        >
          <ArrowLeft />
        </Button>
        <h1 className="text-xl font-semibold">编辑销售记录</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">客户与归属</CardTitle>
        </CardHeader>
        <CardContent>
          <CustomerSection form={form} />
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle className="text-base">内容明细</CardTitle>
        </CardHeader>
        <CardContent>
          <LinesSection form={form} single />
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle className="text-base">协作与人员</CardTitle>
        </CardHeader>
        <CardContent>
          <CollaborationSection form={form} />
        </CardContent>
      </Card>

      <Button type="submit" size="lg" disabled={updateMutation.isPending}>
        {updateMutation.isPending ? '保存中…' : '保存'}
      </Button>
    </form>
  )
}
