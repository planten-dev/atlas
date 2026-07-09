import { useMemo, useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { useForm, type FieldPath } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { requirePerm } from '@/auth/route-guard'
import { useIsMobile } from '@/hooks/use-mobile'
import { activeCategoriesOptions } from '@/hooks/useCategories'
import { useCreateSalesBatch } from '@/hooks/useSales'
import { notify } from '@/lib/notify'
import { toDateParam } from '@/lib/date'
import {
  assembleRecords,
  buildSalesFormSchema,
  SALES_FORM_DEFAULTS,
  type SalesFormValues,
} from './-form/schema'
import { CollaborationSection, CustomerSection, LinesSection } from './-form/sections'

export const Route = createFileRoute('/_app/sales/new')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:records:write'),
  component: SalesNewPage,
})

const STEPS: { title: string; fields: FieldPath<SalesFormValues>[] }[] = [
  {
    title: '客户与归属',
    fields: [
      'customer_id',
      'sale_date',
      'department_id',
      'system_id',
      'store_id',
      'customer_type',
      'deal_type',
      'deal_status',
    ],
  },
  { title: '内容明细', fields: ['lines'] },
  {
    title: '协作与确认',
    fields: ['collaboration_type', 'handler_user_id', 'expert_user_id', 'expert_department_id'],
  },
]

function SalesNewPage() {
  const isMobile = useIsMobile()
  const navigate = useNavigate()
  const { data: categories } = useQuery(activeCategoriesOptions)
  const requiresMap = useMemo(
    () => new Map((categories ?? []).map((c) => [c.id, c.requires_operation_count])),
    [categories],
  )

  const form = useForm<SalesFormValues>({
    resolver: zodResolver(buildSalesFormSchema(requiresMap)),
    defaultValues: { ...SALES_FORM_DEFAULTS, sale_date: toDateParam(new Date()) },
    mode: 'onBlur',
  })

  const createMutation = useCreateSalesBatch()
  const [step, setStep] = useState(0)

  const submit = form.handleSubmit((values) => {
    createMutation.mutate(assembleRecords(values, requiresMap), {
      onSuccess: (outcome) => {
        if (outcome.kind === 'applied') {
          notify.success(`已录入 ${outcome.data.sales_records.length} 条销售记录`)
          void navigate({
            to: '/sales',
            search: { record_group_id: outcome.data.record_group_id },
          })
        } else {
          notify.info('已提交审批,通过后生效')
          void navigate({ to: '/sales', search: {} })
        }
      },
      onError: (error) => notify.error(error),
    })
  })

  const nextStep = async () => {
    const current = STEPS[step]
    if (!current) return
    const valid = await form.trigger(current.fields)
    if (valid) setStep((s) => Math.min(s + 1, STEPS.length - 1))
  }

  if (!isMobile) {
    return (
      <form className="mx-auto flex max-w-4xl flex-col gap-4" onSubmit={submit}>
        <h1 className="text-xl font-semibold">销售录入</h1>
        {STEPS.map((s, i) => (
          <Card key={s.title}>
            <CardHeader>
              <CardTitle className="text-base">{s.title}</CardTitle>
            </CardHeader>
            <CardContent>
              {i === 0 && <CustomerSection form={form} />}
              {i === 1 && <LinesSection form={form} />}
              {i === 2 && <CollaborationSection form={form} />}
            </CardContent>
          </Card>
        ))}
        <Button type="submit" size="lg" disabled={createMutation.isPending}>
          {createMutation.isPending ? '提交中…' : '提交'}
        </Button>
      </form>
    )
  }

  // 移动端 3 步分步:同一 RHF 实例,步进用 trigger() 校验当步字段(设计 §9)
  return (
    <form className="flex flex-col gap-4" onSubmit={submit}>
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">销售录入</h1>
        <span className="text-sm text-muted-foreground">
          {step + 1} / {STEPS.length} · {STEPS[step]?.title}
        </span>
      </div>
      <div className="flex gap-1">
        {STEPS.map((s, i) => (
          <div
            key={s.title}
            className={`h-1 flex-1 rounded-full ${i <= step ? 'bg-primary' : 'bg-muted'}`}
          />
        ))}
      </div>

      <div className={step === 0 ? '' : 'hidden'}>
        <CustomerSection form={form} />
      </div>
      <div className={step === 1 ? '' : 'hidden'}>
        <LinesSection form={form} />
      </div>
      <div className={step === 2 ? '' : 'hidden'}>
        <CollaborationSection form={form} />
      </div>

      <div className="flex gap-2">
        {step > 0 && (
          <Button type="button" variant="outline" className="flex-1" onClick={() => setStep(step - 1)}>
            上一步
          </Button>
        )}
        {step < STEPS.length - 1 ? (
          <Button type="button" className="flex-1" onClick={() => void nextStep()}>
            下一步
          </Button>
        ) : (
          <Button type="submit" className="flex-1" disabled={createMutation.isPending}>
            {createMutation.isPending ? '提交中…' : '提交'}
          </Button>
        )}
      </div>
    </form>
  )
}
