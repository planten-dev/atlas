import { Controller, type UseFormReturn } from 'react-hook-form'
import { useFieldArray } from 'react-hook-form'
import { useQuery } from '@tanstack/react-query'
import { Plus, Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardAction, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Field, FieldError, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { FormDate, FormMoney, FormSelect, FormText, FormTextarea } from '@/components/form/fields'
import { CustomerPicker } from '@/components/pickers/CustomerPicker'
import { ProductPicker } from '@/components/pickers/ProductPicker'
import { UserPicker } from '@/components/pickers/UserPicker'
import { DateTimePicker } from '@/components/pickers/DateTimePicker'
import { AllocationsField } from '@/components/payments/AllocationsField'
import { customerDetailOptions } from '@/hooks/useCustomers'
import { systemsListOptions } from '@/hooks/useSystems'
import { storesListOptions } from '@/hooks/useStores'
import { CUSTOMER_TYPE_LABELS, DEAL_TYPE_LABELS } from '@/lib/labels'
import { EMPTY_LINE, type SalesFormValues } from './schema'

type SalesForm = UseFormReturn<SalesFormValues>

function toOptions(labels: Record<string, string>) {
  return Object.entries(labels).map(([value, label]) => ({ value, label }))
}

function PickerField({
  form,
  name,
  label,
  required,
  children,
}: {
  form: SalesForm
  name: keyof SalesFormValues & string
  label: string
  required?: boolean
  children: (field: { value: string | undefined; onChange: (v: string | undefined) => void }) => React.ReactNode
}) {
  return (
    <Controller
      control={form.control}
      name={name}
      render={({ field, fieldState }) => (
        <Field data-invalid={fieldState.invalid || undefined}>
          <FieldLabel>
            {label}
            {required && <span className="text-destructive">*</span>}
          </FieldLabel>
          {children({
            value: (field.value as string) || undefined,
            onChange: (v) => field.onChange(v ?? ''),
          })}
          {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
        </Field>
      )}
    />
  )
}

/** 选客户后只读显示归属体系/门店(由客户档案决定,后端派生)。 */
function CustomerScopeHint({ customerId }: { customerId: string }) {
  const { data: customer } = useQuery(customerDetailOptions(customerId))
  const { data: systems } = useQuery(systemsListOptions({ page_size: 200 }))
  const { data: stores } = useQuery({
    ...storesListOptions({ system_id: customer?.system_id, page_size: 200 }),
    enabled: Boolean(customer?.system_id),
  })
  if (!customer) return null
  const systemName = systems?.items.find((s) => s.id === customer.system_id)?.name
  const storeName = stores?.items.find((s) => s.id === customer.store_id)?.name
  return (
    <p className="text-xs text-muted-foreground md:col-span-2">
      归属:{systemName ?? '…'} / {storeName ?? '…'}(由客户档案决定)
    </p>
  )
}

/** 第 1 步:记录类型 + 客户与类型。 */
export function CustomerSection({ form }: { form: SalesForm }) {
  const recordType = form.watch('record_type')
  const customerId = form.watch('customer_id')

  return (
    <div className="grid gap-4 md:grid-cols-2">
      <Controller
        control={form.control}
        name="record_type"
        render={({ field }) => (
          <Field className="md:col-span-2">
            <FieldLabel>
              记录类型<span className="text-destructive">*</span>
            </FieldLabel>
            <Tabs
              value={field.value}
              onValueChange={(value) => {
                field.onChange(value)
                if (value === 'service') {
                  // 服务记录:行金额锁 0、清次数
                  form.getValues('lines').forEach((_, index) => {
                    form.setValue(`lines.${index}.receivable_amount`, '0.00')
                    form.setValue(`lines.${index}.operation_total_count`, undefined)
                  })
                }
              }}
            >
              <TabsList>
                <TabsTrigger value="sale">销售</TabsTrigger>
                <TabsTrigger value="service">服务</TabsTrigger>
              </TabsList>
            </Tabs>
          </Field>
        )}
      />
      <PickerField form={form} name="customer_id" label="客户" required>
        {(field) => <CustomerPicker value={field.value} onChange={field.onChange} />}
      </PickerField>
      <FormDate control={form.control} name="record_date" label="成交日期" required />
      {customerId && <CustomerScopeHint customerId={customerId} />}
      <FormSelect
        control={form.control}
        name="customer_type"
        label="客户类型"
        required
        options={toOptions(CUSTOMER_TYPE_LABELS)}
      />
      <FormSelect
        control={form.control}
        name="deal_type"
        label="成交类型"
        required
        options={toOptions(DEAL_TYPE_LABELS)}
      />
      {recordType === 'service' && (
        <p className="text-xs text-muted-foreground md:col-span-2">
          服务记录不涉及收款,明细金额固定为 0。
        </p>
      )}
    </div>
  )
}

/** 第 2 步:产品明细行(动态增删)。 */
export function LinesSection({ form }: { form: SalesForm }) {
  const { fields, append, remove } = useFieldArray({ control: form.control, name: 'lines' })
  const recordType = form.watch('record_type')
  const lines = form.watch('lines')
  const isService = recordType === 'service'

  return (
    <div className="flex flex-col gap-3">
      {fields.map((item, index) => {
        const requiresCount = !isService && (lines[index]?.requires_operation_count ?? false)
        return (
          <Card key={item.id} className="gap-3 py-4">
            <CardHeader>
              <CardTitle className="text-sm text-muted-foreground">明细 {index + 1}</CardTitle>
              {fields.length > 1 && (
                <CardAction>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="-my-1.5 text-muted-foreground hover:text-destructive"
                    onClick={() => remove(index)}
                    aria-label={`删除明细 ${index + 1}`}
                  >
                    <Trash2 />
                  </Button>
                </CardAction>
              )}
            </CardHeader>
            <CardContent className="grid gap-4 md:grid-cols-3">
              <Controller
                control={form.control}
                name={`lines.${index}.product_id`}
                render={({ field, fieldState }) => (
                  <Field data-invalid={fieldState.invalid || undefined}>
                    <FieldLabel>
                      产品<span className="text-destructive">*</span>
                    </FieldLabel>
                    <ProductPicker
                      value={field.value || undefined}
                      onChange={(id, product) => {
                        field.onChange(id ?? '')
                        // 快照产品名与次数规则;换产品时清残留次数
                        form.setValue(
                          `lines.${index}.requires_operation_count`,
                          product?.requires_operation_count ?? false,
                        )
                        form.setValue(`lines.${index}.operation_total_count`, undefined)
                        if (product) {
                          form.setValue(`lines.${index}.item_name`, product.name, {
                            shouldValidate: Boolean(product),
                          })
                        }
                      }}
                    />
                    {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
                  </Field>
                )}
              />
              <FormText
                control={form.control}
                name={`lines.${index}.item_name`}
                label="项目名称"
                required
              />
              {isService ? (
                <Field>
                  <FieldLabel>应收金额</FieldLabel>
                  <Input value="0.00" disabled />
                </Field>
              ) : (
                <FormMoney
                  control={form.control}
                  name={`lines.${index}.receivable_amount`}
                  label="应收金额"
                  required
                />
              )}
              {requiresCount && (
                <Controller
                  control={form.control}
                  name={`lines.${index}.operation_total_count`}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid || undefined}>
                      <FieldLabel>
                        可操作次数<span className="text-destructive">*</span>
                      </FieldLabel>
                      <Input
                        type="number"
                        min={1}
                        inputMode="numeric"
                        value={field.value ?? ''}
                        onChange={(e) =>
                          field.onChange(e.target.value === '' ? undefined : Number(e.target.value))
                        }
                        aria-invalid={fieldState.invalid || undefined}
                      />
                      {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
                    </Field>
                  )}
                />
              )}
              <FormText control={form.control} name={`lines.${index}.remark`} label="行备注" />
            </CardContent>
          </Card>
        )
      })}
      <Button
        type="button"
        variant="outline"
        onClick={() =>
          append(isService ? { ...EMPTY_LINE, receivable_amount: '0.00' } : { ...EMPTY_LINE })
        }
      >
        <Plus />
        添加明细
      </Button>
    </div>
  )
}

/** 第 3 步:付款(仅销售)与人员。 */
export function PaymentSection({ form }: { form: SalesForm }) {
  const recordType = form.watch('record_type')
  const allocations = form.watch('payment.allocations')
  const allocationsError = form.formState.errors.payment?.allocations?.root?.message ??
    (form.formState.errors.payment?.allocations as { message?: string } | undefined)?.message

  return (
    <div className="flex flex-col gap-4">
      <div className="grid gap-4 md:grid-cols-2">
        <PickerField form={form} name="handler_user_id" label="处理人" required>
          {(field) => <UserPicker value={field.value} onChange={field.onChange} />}
        </PickerField>
        <PickerField form={form} name="expert_user_id" label="专家">
          {(field) => <UserPicker value={field.value} onChange={field.onChange} placeholder="选择专家(选填)" />}
        </PickerField>
        <PickerField form={form} name="consultant_user_id" label="咨询师">
          {(field) => <UserPicker value={field.value} onChange={field.onChange} placeholder="选择咨询师(选填)" />}
        </PickerField>
        <PickerField form={form} name="doctor_user_id" label="医生">
          {(field) => <UserPicker value={field.value} onChange={field.onChange} placeholder="选择医生(选填)" />}
        </PickerField>
      </div>
      <FormTextarea control={form.control} name="remark" label="记录备注" />

      {recordType === 'sale' && (
        <div className="flex flex-col gap-4 rounded-md border p-4">
          <h4 className="text-sm font-medium">首次付款</h4>
          <div className="grid gap-4 md:grid-cols-2">
            <FormMoney control={form.control} name="payment.paid_amount" label="首款金额" required />
            <Controller
              control={form.control}
              name="payment.paid_at"
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
          </div>
          <AllocationsField
            control={form.control}
            name="payment.allocations"
            rows={allocations ?? []}
            error={allocationsError}
          />
          <FormTextarea control={form.control} name="payment.remark" label="付款备注" />
        </div>
      )}
    </div>
  )
}
