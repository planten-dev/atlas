import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Field, FieldError, FieldLabel } from '@/components/ui/field'
import { FormMoney, FormTextarea } from '@/components/form/fields'
import { DateTimePicker } from '@/components/pickers/DateTimePicker'
import { AllocationsField } from '@/components/payments/AllocationsField'
import { allocationSchema, ratioToBasisPoints } from '@/routes/_app/sales/-form/schema'
import { useCollectPayment } from '@/hooks/usePayments'
import { compareAmounts, formatAmount } from '@/lib/money'
import { toLocalDateTimeInput } from '@/lib/date'
import { notify } from '@/lib/notify'

const AMOUNT_PATTERN = /^\d{1,10}(\.\d{1,2})?$/

function buildSchema(outstanding: string) {
  return z
    .object({
      paid_amount: z
        .string()
        .min(1, '请填写回款金额')
        .regex(AMOUNT_PATTERN, '金额格式:最多 10 位整数 + 2 位小数'),
      paid_at: z.string().min(1, '请选择支付时间'),
      allocations: z.array(allocationSchema).min(1, '至少一条业绩分配'),
      remark: z.string().max(2000, '备注最长 2000 字').optional(),
    })
    .superRefine((values, ctx) => {
      if (AMOUNT_PATTERN.test(values.paid_amount)) {
        if (compareAmounts(values.paid_amount, '0') <= 0) {
          ctx.addIssue({ code: 'custom', path: ['paid_amount'], message: '回款金额必须大于 0' })
        } else if (compareAmounts(values.paid_amount, outstanding) > 0) {
          ctx.addIssue({
            code: 'custom',
            path: ['paid_amount'],
            message: `回款金额不能超过未收金额 ${formatAmount(outstanding)}`,
          })
        }
      }
      const points = values.allocations.map((a) => ratioToBasisPoints(a.allocation_ratio))
      if (points.every((p): p is number => p !== null)) {
        const sum = points.reduce((acc, p) => acc + p, 0)
        if (sum !== 10000) {
          ctx.addIssue({ code: 'custom', path: ['allocations'], message: '分配比例合计必须等于 100%' })
        }
      }
      const guides = values.allocations.map((a) => a.guide_user_id).filter(Boolean)
      if (new Set(guides).size !== guides.length) {
        ctx.addIssue({ code: 'custom', path: ['allocations'], message: '同一导购不能重复分配' })
      }
    })
}

type CollectFormValues = z.infer<ReturnType<typeof buildSchema>>

/** 登记回款:金额 ≤ 未收(后端 409 payment_exceeds_outstanding 兜底)。 */
export function CollectPaymentDialog({
  open,
  onOpenChange,
  salesRecordId,
  outstanding,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  salesRecordId: string
  outstanding: string
}) {
  const form = useForm<CollectFormValues>({
    resolver: zodResolver(buildSchema(outstanding)),
    defaultValues: {
      paid_amount: '',
      paid_at: toLocalDateTimeInput(new Date()),
      allocations: [{ guide_user_id: '', allocation_ratio: '100' }],
      remark: '',
    },
    mode: 'onBlur',
  })
  const collectMutation = useCollectPayment()
  const allocations = form.watch('allocations')
  const allocationsError =
    form.formState.errors.allocations?.root?.message ??
    (form.formState.errors.allocations as { message?: string } | undefined)?.message

  const submit = form.handleSubmit((values) => {
    collectMutation.mutate(
      {
        sales_record_id: salesRecordId,
        paid_amount: values.paid_amount,
        paid_at: new Date(values.paid_at).toISOString(),
        allocations: values.allocations.map((allocation) => ({
          guide_user_id: allocation.guide_user_id,
          allocation_ratio: allocation.allocation_ratio,
        })),
        remark: values.remark || null,
      },
      {
        onSuccess: () => {
          notify.success('回款已登记')
          onOpenChange(false)
          form.reset()
        },
        onError: (error) => notify.error(error),
      },
    )
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="grid max-h-[85vh] grid-rows-[auto_1fr] gap-4 sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>登记回款</DialogTitle>
        </DialogHeader>
        <form
          className="-mx-2 flex flex-col gap-4 overflow-y-auto px-2 pb-1"
          onSubmit={(e) => {
            e.stopPropagation()
            void submit(e)
          }}
        >
          <p className="text-sm text-muted-foreground">
            当前未收金额:<span className="font-medium tabular-nums">{formatAmount(outstanding)}</span>
          </p>
          <FormMoney control={form.control} name="paid_amount" label="回款金额" required />
          <Controller
            control={form.control}
            name="paid_at"
            render={({ field, fieldState }) => (
              <Field data-invalid={fieldState.invalid || undefined}>
                <FieldLabel>
                  支付时间<span className="text-destructive">*</span>
                </FieldLabel>
                <DateTimePicker
                  value={field.value || undefined}
                  onChange={field.onChange}
                  aria-invalid={fieldState.invalid || undefined}
                />
                {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
              </Field>
            )}
          />
          <AllocationsField
            control={form.control}
            name="allocations"
            rows={allocations ?? []}
            error={allocationsError}
          />
          <FormTextarea control={form.control} name="remark" label="备注" />
          <Button type="submit" disabled={collectMutation.isPending}>
            {collectMutation.isPending ? '提交中…' : '登记回款'}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  )
}
