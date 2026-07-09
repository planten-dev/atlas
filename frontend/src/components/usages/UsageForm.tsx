import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Field, FieldError, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { FormTextarea } from '@/components/form/fields'
import { UserPicker } from '@/components/pickers/UserPicker'
import { CustomerPicker } from '@/components/pickers/CustomerPicker'
import { CustomerName } from '@/components/customers/CustomerName'
import { salesDetailOptions, salesListOptions } from '@/hooks/useSales'
import { useCreateUsage } from '@/hooks/useUsages'
import { formatDate } from '@/lib/date'
import { notify } from '@/lib/notify'
import { outcomeMessage } from '@/hooks/mutation-result'

const usageFormSchema = z.object({
  sales_record_id: z.string().min(1, '请选择销售记录'),
  operated_at: z.string().min(1, '请选择操作时间'),
  operator_user_id: z.string().min(1, '请选择操作人'),
  doctor_user_id: z.string().optional(),
  operation_count: z.number().int('必须为整数').min(1, '至少 1 次'),
  remark: z.string().max(2000, '备注过长').optional(),
})

type UsageFormValues = z.infer<typeof usageFormSchema>

function toLocalDateTimeInput(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`
}

/**
 * 登记耗用表单(页面与弹窗共用):
 * lockedSalesRecordId 提供时锁定记录并显示剩余次数,否则先选客户再选记录。
 */
export function UsageForm({
  lockedSalesRecordId,
  onSuccess,
}: {
  lockedSalesRecordId?: string
  onSuccess: (salesRecordId: string) => void
}) {
  const form = useForm<UsageFormValues>({
    resolver: zodResolver(usageFormSchema),
    defaultValues: {
      sales_record_id: lockedSalesRecordId ?? '',
      operated_at: toLocalDateTimeInput(new Date()),
      operator_user_id: '',
      doctor_user_id: '',
      operation_count: 1,
      remark: '',
    },
  })

  const createMutation = useCreateUsage()
  const selectedRecordId = form.watch('sales_record_id')

  const submit = form.handleSubmit((values) => {
    createMutation.mutate(
      {
        sales_record_id: values.sales_record_id,
        operated_at: new Date(values.operated_at).toISOString(),
        operator_user_id: values.operator_user_id,
        doctor_user_id: values.doctor_user_id || null,
        operation_count: values.operation_count,
        remark: values.remark || null,
      },
      {
        onSuccess: (outcome) => {
          notify.success(outcomeMessage(outcome, '耗用已登记'))
          onSuccess(values.sales_record_id)
        },
        onError: (error) => notify.error(error),
      },
    )
  })

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(e) => {
        e.stopPropagation()
        void submit(e)
      }}
    >
      {lockedSalesRecordId ? (
        <LockedRecordCard salesRecordId={lockedSalesRecordId} />
      ) : (
        <RecordSelector
          value={selectedRecordId || undefined}
          onChange={(id) => form.setValue('sales_record_id', id ?? '', { shouldValidate: true })}
          error={form.formState.errors.sales_record_id?.message}
        />
      )}

      <Controller
        control={form.control}
        name="operated_at"
        render={({ field, fieldState }) => (
          <Field data-invalid={fieldState.invalid || undefined}>
            <FieldLabel htmlFor="operated_at">
              操作时间<span className="text-destructive">*</span>
            </FieldLabel>
            <Input id="operated_at" type="datetime-local" {...field} />
            {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
          </Field>
        )}
      />

      <Controller
        control={form.control}
        name="operator_user_id"
        render={({ field, fieldState }) => (
          <Field data-invalid={fieldState.invalid || undefined}>
            <FieldLabel>
              操作人<span className="text-destructive">*</span>
            </FieldLabel>
            <UserPicker value={field.value || undefined} onChange={(v) => field.onChange(v ?? '')} />
            {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
          </Field>
        )}
      />

      <Controller
        control={form.control}
        name="doctor_user_id"
        render={({ field }) => (
          <Field>
            <FieldLabel>医生</FieldLabel>
            <UserPicker
              value={field.value || undefined}
              onChange={(v) => field.onChange(v ?? '')}
              placeholder="选择医生(选填)"
            />
          </Field>
        )}
      />

      <Controller
        control={form.control}
        name="operation_count"
        render={({ field, fieldState }) => (
          <Field data-invalid={fieldState.invalid || undefined}>
            <FieldLabel htmlFor="operation_count">
              操作次数<span className="text-destructive">*</span>
            </FieldLabel>
            <Input
              id="operation_count"
              type="number"
              min={1}
              inputMode="numeric"
              value={field.value ?? ''}
              onChange={(e) => field.onChange(e.target.value === '' ? undefined : Number(e.target.value))}
            />
            {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
          </Field>
        )}
      />

      <FormTextarea control={form.control} name="remark" label="备注" />

      <Button type="submit" size="lg" disabled={createMutation.isPending}>
        {createMutation.isPending ? '提交中…' : '登记'}
      </Button>
    </form>
  )
}

/** 列表页/详情页的登记耗用弹窗。 */
export function UsageFormDialog({
  open,
  onOpenChange,
  lockedSalesRecordId,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  lockedSalesRecordId?: string
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {/* 外壳固定圆角,内容区单独滚动,滚动条不压边框/关闭按钮 */}
      <DialogContent className="grid max-h-[85vh] grid-rows-[auto_1fr] gap-4 sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>登记耗用</DialogTitle>
        </DialogHeader>
        <div className="-mx-2 overflow-y-auto px-2 pb-1">
          <UsageForm
            lockedSalesRecordId={lockedSalesRecordId}
            onSuccess={() => onOpenChange(false)}
          />
        </div>
      </DialogContent>
    </Dialog>
  )
}

/** 锁定记录:显示客户/日期与剩余次数。 */
function LockedRecordCard({ salesRecordId }: { salesRecordId: string }) {
  const { data: record } = useQuery(salesDetailOptions(salesRecordId))
  return (
    <Card>
      <CardContent className="flex flex-col gap-1 py-3 text-sm">
        {record ? (
          <>
            <p>
              <CustomerName customerId={record.customer_id} /> · {formatDate(record.sale_date)}
            </p>
            {record.operation_count ? (
              <p className="text-muted-foreground">
                次数账户:剩余 <span className="font-medium">{record.operation_count.remaining_count}</span> /{' '}
                {record.operation_count.total_count} 次
              </p>
            ) : (
              <p className="text-destructive">该记录无次数账户,无法登记耗用</p>
            )}
          </>
        ) : (
          <p className="text-muted-foreground">加载记录中…</p>
        )}
      </CardContent>
    </Card>
  )
}

/** 未锁定:先选客户 → 列出该客户有次数账户的记录。 */
function RecordSelector({
  value,
  onChange,
  error,
}: {
  value: string | undefined
  onChange: (id: string | undefined) => void
  error?: string
}) {
  const [customerId, setCustomerId] = useState<string | undefined>(undefined)
  const recordsQuery = useQuery({
    ...salesListOptions({ customer_id: customerId, status_filter: 'active', page_size: 50 }),
    enabled: Boolean(customerId),
  })
  const records = (recordsQuery.data?.items ?? []).filter((r) => r.operation_count)

  return (
    <div className="flex flex-col gap-2">
      <Field data-invalid={error ? true : undefined}>
        <FieldLabel>
          客户<span className="text-destructive">*</span>
        </FieldLabel>
        <CustomerPicker
          value={customerId}
          onChange={(v) => {
            setCustomerId(v)
            onChange(undefined)
          }}
          allowCreate={false}
        />
        {error && <FieldError>{error}</FieldError>}
      </Field>

      {customerId && (
        <div className="flex flex-col gap-1">
          {recordsQuery.isLoading ? (
            <p className="text-sm text-muted-foreground">加载记录中…</p>
          ) : records.length === 0 ? (
            <p className="text-sm text-muted-foreground">该客户没有可耗用的销售记录</p>
          ) : (
            records.map((record) => (
              <button
                key={record.id}
                type="button"
                onClick={() => onChange(record.id)}
                className={`rounded-md border p-2 text-left text-sm transition-colors ${
                  value === record.id ? 'border-primary bg-primary/5' : 'hover:bg-accent/50'
                }`}
              >
                <p>{formatDate(record.sale_date)}</p>
                <p className="text-muted-foreground">
                  剩余 {record.operation_count?.remaining_count} / {record.operation_count?.total_count} 次
                </p>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  )
}
