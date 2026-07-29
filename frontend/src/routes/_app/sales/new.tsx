import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useForm, type FieldPath } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { requirePerm } from '@/auth/route-guard'
import { useIsMobile } from '@/hooks/use-mobile'
import { useCreateDeal, useCreatePreService, useCreateDebtCollection } from '@/hooks/useSales'
import { notify } from '@/lib/notify'
import { toDateParam } from '@/lib/date'
import {
  assembleDealRequest,
  assemblePreServiceRequest,
  assembleDebtCollectionRequest,
  salesFormSchema,
  SALES_FORM_DEFAULTS,
  type SalesFormValues,
} from './-form/schema'
import { CustomerSection, LinesSection, PaymentSection } from './-form/sections'

export const Route = createFileRoute('/_app/sales/new')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:records:write'),
  component: SalesNewPage,
})

const STEPS: { title: string; fields: FieldPath<SalesFormValues>[] }[] = [
  {
    title: '客户与类型',
    fields: ['record_type', 'customer_id', 'record_date', 'customer_type', 'deal_type'],
  },
  { title: '产品明细', fields: ['lines'] },
  {
    title: '付款与人员',
    fields: ['handler_user_id', 'total_amount', 'received_amount', 'allocations'],
  },
]

function SalesNewPage() {
  const isMobile = useIsMobile()
  const navigate = useNavigate()

  const form = useForm<SalesFormValues>({
    resolver: zodResolver(salesFormSchema),
    defaultValues: {
      ...SALES_FORM_DEFAULTS,
      record_date: toDateParam(new Date()),
    },
    mode: 'onBlur',
  })

  const createDeal = useCreateDeal()
  const createPreService = useCreatePreService()
  const createDebtCollection = useCreateDebtCollection()
  const isPending = createDeal.isPending || createPreService.isPending || createDebtCollection.isPending
  const [step, setStep] = useState(0)

  const submit = form.handleSubmit((values) => {
    const options = {
      onSuccess: (
        outcome:
          | { kind: 'applied'; data: { id: string } }
          | { kind: 'submitted'; eventId: string },
      ) => {
        if (outcome.kind === 'applied') {
          notify.success('已录入销售记录')
          void navigate({ to: '/sales/$salesId', params: { salesId: outcome.data.id } })
        } else {
          notify.info('已提交审批,通过后生效')
          void navigate({ to: '/sales', search: {} })
        }
      },
      onError: (error: Error) => notify.error(error),
    }
    if (values.record_type === 'deal') {
      createDeal.mutate(assembleDealRequest(values), options)
    } else if (values.record_type === 'pre_service') {
      createPreService.mutate(assemblePreServiceRequest(values), options)
    } else {
      createDebtCollection.mutate(assembleDebtCollectionRequest(values), options)
    }
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
        <h1 className="text-xl font-semibold">销售记录录入</h1>
        {STEPS.map((s, i) => (
          <Card key={s.title}>
            <CardHeader>
              <CardTitle className="text-base">{s.title}</CardTitle>
            </CardHeader>
            <CardContent>
              {i === 0 && <CustomerSection form={form} />}
              {i === 1 && <LinesSection form={form} />}
              {i === 2 && <PaymentSection form={form} />}
            </CardContent>
          </Card>
        ))}
        <Button type="submit" size="lg" disabled={isPending}>
          {isPending ? '提交中…' : '提交'}
        </Button>
      </form>
    )
  }

  // 移动端 3 步分步:同一 RHF 实例,步进用 trigger() 校验当步字段
  return (
    <form className="flex flex-col gap-4" onSubmit={submit}>
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">销售记录录入</h1>
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
        <PaymentSection form={form} />
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
          <Button type="submit" className="flex-1" disabled={isPending}>
            {isPending ? '提交中…' : '提交'}
          </Button>
        )}
      </div>
    </form>
  )
}
