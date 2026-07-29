import { useNavigate } from '@tanstack/react-router'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Separator } from '@/components/ui/separator'
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
} from './schema'
import { CustomerSection, LinesSection, PaymentSection } from './sections'

function formDefaults(): SalesFormValues {
  return {
    ...SALES_FORM_DEFAULTS,
    record_date: toDateParam(new Date()),
  }
}

/** 桌面端销售/服务录入弹窗(移动端走 /sales/new 分步页)。 */
export function SalesFormDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const navigate = useNavigate()
  const form = useForm<SalesFormValues>({
    resolver: zodResolver(salesFormSchema),
    defaultValues: formDefaults(),
    mode: 'onBlur',
  })

  const createDeal = useCreateDeal()
  const createPreService = useCreatePreService()
  const createDebtCollection = useCreateDebtCollection()
  const isPending = createDeal.isPending || createPreService.isPending || createDebtCollection.isPending

  const submit = form.handleSubmit((values) => {
    const options = {
      onSuccess: (outcome: { kind: 'applied'; data: { id: string } } | { kind: 'submitted'; eventId: string }) => {
        onOpenChange(false)
        form.reset(formDefaults())
        if (outcome.kind === 'applied') {
          notify.success('已录入销售记录')
          void navigate({ to: '/sales/$salesId', params: { salesId: outcome.data.id } })
        } else {
          notify.info('已提交审批,通过后生效')
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

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {/* 外壳固定圆角,内容区单独滚动,滚动条不压边框/关闭按钮 */}
      <DialogContent className="grid max-h-[88vh] grid-rows-[auto_1fr] gap-4 sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>销售记录录入</DialogTitle>
        </DialogHeader>
        <form
          className="-mx-2 flex flex-col gap-5 overflow-y-auto px-2 pb-1"
          onSubmit={(e) => {
            e.stopPropagation()
            void submit(e)
          }}
        >
          <section className="flex flex-col gap-3">
            <h3 className="text-sm font-medium text-muted-foreground">客户与类型</h3>
            <CustomerSection form={form} />
          </section>
          <Separator />
          <section className="flex flex-col gap-3">
            <h3 className="text-sm font-medium text-muted-foreground">产品明细</h3>
            <LinesSection form={form} />
          </section>
          <Separator />
          <section className="flex flex-col gap-3">
            <h3 className="text-sm font-medium text-muted-foreground">付款与人员</h3>
            <PaymentSection form={form} />
          </section>
          <Button type="submit" size="lg" disabled={isPending}>
            {isPending ? '提交中…' : '提交'}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}
