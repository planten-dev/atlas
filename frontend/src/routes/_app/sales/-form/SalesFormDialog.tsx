import { useMemo } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
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
import { activeCategoriesOptions } from '@/hooks/useCategories'
import { useCreateSalesBatch } from '@/hooks/useSales'
import { notify } from '@/lib/notify'
import { toDateParam } from '@/lib/date'
import {
  assembleRecords,
  buildSalesFormSchema,
  SALES_FORM_DEFAULTS,
  type SalesFormValues,
} from './schema'
import { CollaborationSection, CustomerSection, LinesSection } from './sections'

/** 桌面端销售录入弹窗(移动端走 /sales/new 分步页)。 */
export function SalesFormDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
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

  const submit = form.handleSubmit((values) => {
    createMutation.mutate(assembleRecords(values, requiresMap), {
      onSuccess: (outcome) => {
        onOpenChange(false)
        form.reset({ ...SALES_FORM_DEFAULTS, sale_date: toDateParam(new Date()) })
        if (outcome.kind === 'applied') {
          notify.success(`已录入 ${outcome.data.sales_records.length} 条销售记录`)
          void navigate({
            to: '/sales',
            search: { record_group_id: outcome.data.record_group_id },
          })
        } else {
          notify.info('已提交审批,通过后生效')
        }
      },
      onError: (error) => notify.error(error),
    })
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {/* 外壳固定圆角,内容区单独滚动,滚动条不压边框/关闭按钮 */}
      <DialogContent className="grid max-h-[88vh] grid-rows-[auto_1fr] gap-4 sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>销售录入</DialogTitle>
        </DialogHeader>
        <form
          className="-mx-2 flex flex-col gap-5 overflow-y-auto px-2 pb-1"
          onSubmit={(e) => {
            e.stopPropagation()
            void submit(e)
          }}
        >
          <section className="flex flex-col gap-3">
            <h3 className="text-sm font-medium text-muted-foreground">客户与归属</h3>
            <CustomerSection form={form} />
          </section>
          <Separator />
          <section className="flex flex-col gap-3">
            <h3 className="text-sm font-medium text-muted-foreground">内容明细</h3>
            <LinesSection form={form} />
          </section>
          <Separator />
          <section className="flex flex-col gap-3">
            <h3 className="text-sm font-medium text-muted-foreground">协作与人员</h3>
            <CollaborationSection form={form} />
          </section>
          <Button type="submit" size="lg" disabled={createMutation.isPending}>
            {createMutation.isPending ? '提交中…' : '提交'}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}
