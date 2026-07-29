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
import { DateTimePicker } from '@/components/pickers/DateTimePicker'
import { CustomerPicker } from '@/components/pickers/CustomerPicker'
import { CustomerName } from '@/components/customers/CustomerName'
import {
  salesDetailOptions,
  salesListOptions,
  type SalesRecordLineResponse,
  type SalesRecordResponse,
} from '@/hooks/useSales'
import { useCreateUsage } from '@/hooks/useUsages'
import { formatDate, toLocalDateTimeInput } from '@/lib/date'
import { notify } from '@/lib/notify'
import { outcomeMessage } from '@/hooks/mutation-result'

const usageFormSchema = z.object({
  sales_record_line_id: z.string().min(1, '请选择明细行'),
  operated_at: z.string().min(1, '请选择操作时间'),
  operator_user_id: z.string().min(1, '请选择操作人'),
  doctor_user_id: z.string().optional(),
  operation_count: z.number().int('必须为整数').min(1, '至少 1 次'),
  remark: z.string().max(2000, '备注过长').optional(),
})

type UsageFormValues = z.infer<typeof usageFormSchema>

/** 有有效次数账户、可登记耗用的明细行。 */
function eligibleLines(record: SalesRecordResponse): SalesRecordLineResponse[] {
  return record.lines.filter(
    (line) => line.status === 'active' && line.operation_count?.status === 'active',
  )
}

/**
 * 登记耗用表单(页面与弹窗共用),三种模式:
 * - lockedLineId + lockedSalesRecordId:锁定明细行(详情页明细行进入)
 * - lockedSalesRecordId:锁定记录,行内选择(深链)
 * - 都不传:先选客户 → 记录 → 明细行
 */
export function UsageForm({
  lockedSalesRecordId,
  lockedLineId,
  onSuccess,
}: {
  lockedSalesRecordId?: string
  lockedLineId?: string
  onSuccess: (salesRecordId: string) => void
}) {
  const form = useForm<UsageFormValues>({
    resolver: zodResolver(usageFormSchema),
    defaultValues: {
      sales_record_line_id: lockedLineId ?? '',
      operated_at: toLocalDateTimeInput(new Date()),
      operator_user_id: '',
      doctor_user_id: '',
      operation_count: 1,
      remark: '',
    },
  })

  const createMutation = useCreateUsage()
  const selectedLineId = form.watch('sales_record_line_id')
  // 自由模式下选行时记录所属记录 id,提交成功后跳详情
  const [freeRecordId, setFreeRecordId] = useState<string | undefined>(undefined)

  const submit = form.handleSubmit((values) => {
    createMutation.mutate(
      {
        sales_record_line_id: values.sales_record_line_id,
        operated_at: new Date(values.operated_at).toISOString(),
        operator_user_id: values.operator_user_id,
        doctor_user_id: values.doctor_user_id || null,
        operation_count: values.operation_count,
        remark: values.remark || null,
      },
      {
        onSuccess: (outcome) => {
          notify.success(outcomeMessage(outcome, '耗用已登记'))
          const recordId =
            lockedSalesRecordId ??
            freeRecordId ??
            (outcome.kind === 'applied' ? outcome.data.sales_record_id : undefined)
          if (recordId) onSuccess(recordId)
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
        <LockedRecordLines
          salesRecordId={lockedSalesRecordId}
          lockedLineId={lockedLineId}
          value={selectedLineId || undefined}
          onChange={(id) =>
            form.setValue('sales_record_line_id', id ?? '', { shouldValidate: true })
          }
          error={form.formState.errors.sales_record_line_id?.message}
        />
      ) : (
        <LineSelector
          value={selectedLineId || undefined}
          onChange={(lineId, recordId) => {
            setFreeRecordId(recordId)
            form.setValue('sales_record_line_id', lineId ?? '', { shouldValidate: true })
          }}
          error={form.formState.errors.sales_record_line_id?.message}
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
            <DateTimePicker
              id="operated_at"
              value={field.value || undefined}
              onChange={field.onChange}
              aria-invalid={fieldState.invalid || undefined}
            />
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
  lockedLineId,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  lockedSalesRecordId?: string
  lockedLineId?: string
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
            lockedLineId={lockedLineId}
            onSuccess={() => onOpenChange(false)}
          />
        </div>
      </DialogContent>
    </Dialog>
  )
}

function LineButton({
  line,
  selected,
  onClick,
}: {
  line: SalesRecordLineResponse
  selected: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-md border p-2 text-left text-sm transition-colors ${
        selected ? 'border-primary bg-primary/5' : 'hover:bg-accent/50'
      }`}
    >
      <p>{line.item_name}</p>
      <p className="text-muted-foreground">
        剩余 {line.operation_count?.remaining_count} / {line.operation_count?.total_count} 次
      </p>
    </button>
  )
}

/** 记录锁定:显示记录信息 + 可耗用明细行选择(单行锁定时只显示该行)。 */
function LockedRecordLines({
  salesRecordId,
  lockedLineId,
  value,
  onChange,
  error,
}: {
  salesRecordId: string
  lockedLineId?: string
  value: string | undefined
  onChange: (id: string | undefined) => void
  error?: string
}) {
  const { data: record } = useQuery(salesDetailOptions(salesRecordId))
  if (!record) {
    return <p className="text-sm text-muted-foreground">加载记录中…</p>
  }
  const lines = eligibleLines(record).filter(
    (line) => !lockedLineId || line.id === lockedLineId,
  )
  return (
    <div className="flex flex-col gap-2">
      <Card>
        <CardContent className="py-3 text-sm">
          <CustomerName customerId={record.customer_id} /> · {formatDate(record.record_date)}
        </CardContent>
      </Card>
      {lines.length === 0 ? (
        <p className="text-sm text-destructive">该记录没有可耗用的明细行</p>
      ) : (
        <Field data-invalid={error ? true : undefined}>
          <FieldLabel>
            明细行<span className="text-destructive">*</span>
          </FieldLabel>
          <div className="flex flex-col gap-1">
            {lines.map((line) => (
              <LineButton
                key={line.id}
                line={line}
                selected={value === line.id}
                onClick={() => onChange(line.id)}
              />
            ))}
          </div>
          {error && <FieldError>{error}</FieldError>}
        </Field>
      )}
    </div>
  )
}

/** 自由模式:客户 → 销售记录 → 可耗用明细行(两级展开)。 */
function LineSelector({
  value,
  onChange,
  error,
}: {
  value: string | undefined
  onChange: (lineId: string | undefined, recordId?: string) => void
  error?: string
}) {
  const [customerId, setCustomerId] = useState<string | undefined>(undefined)
  const recordsQuery = useQuery({
    ...salesListOptions({
      customer_id: customerId,
      status_filter: 'active',
      record_type: 'deal',
      page_size: 50,
    }),
    enabled: Boolean(customerId),
  })
  const records = (recordsQuery.data?.items ?? []).filter(
    (record) => eligibleLines(record).length > 0,
  )

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
        <div className="flex flex-col gap-2">
          {recordsQuery.isLoading ? (
            <p className="text-sm text-muted-foreground">加载记录中…</p>
          ) : records.length === 0 ? (
            <p className="text-sm text-muted-foreground">该客户没有可耗用的销售记录</p>
          ) : (
            records.map((record) => (
              <div key={record.id} className="flex flex-col gap-1 rounded-md border p-2">
                <p className="text-sm text-muted-foreground">{formatDate(record.record_date)}</p>
                {eligibleLines(record).map((line) => (
                  <LineButton
                    key={line.id}
                    line={line}
                    selected={value === line.id}
                    onClick={() => onChange(line.id, record.id)}
                  />
                ))}
              </div>
            ))
          )}
        </div>
      )}
    </div>
  )
}
