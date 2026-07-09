import { Controller, useFieldArray, type UseFormReturn } from 'react-hook-form'
import { useQuery } from '@tanstack/react-query'
import { Plus, Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Field, FieldError, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { FormDate, FormMoney, FormSelect } from '@/components/form/fields'
import { CustomerPicker } from '@/components/pickers/CustomerPicker'
import { DepartmentPicker } from '@/components/pickers/DepartmentPicker'
import { SystemPicker } from '@/components/pickers/SystemPicker'
import { StorePicker } from '@/components/pickers/StorePicker'
import { CategoryPicker } from '@/components/pickers/CategoryPicker'
import { UserPicker } from '@/components/pickers/UserPicker'
import { activeCategoriesOptions } from '@/hooks/useCategories'
import {
  COLLABORATION_TYPE_LABELS,
  CUSTOMER_TYPE_LABELS,
  DEAL_STATUS_LABELS,
  DEAL_TYPE_LABELS,
} from '@/lib/labels'
import type { SalesFormValues } from './schema'

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

/** 第 1 步:客户与归属。 */
export function CustomerSection({ form }: { form: SalesForm }) {
  const departmentId = form.watch('department_id')
  const systemId = form.watch('system_id')

  return (
    <div className="grid gap-4 md:grid-cols-2">
      <PickerField form={form} name="customer_id" label="客户" required>
        {(field) => <CustomerPicker value={field.value} onChange={field.onChange} />}
      </PickerField>
      <FormDate control={form.control} name="sale_date" label="成交日期" required />
      <PickerField form={form} name="department_id" label="部门" required>
        {(field) => (
          <DepartmentPicker
            value={field.value}
            onChange={(v) => {
              field.onChange(v)
              form.setValue('system_id', '')
              form.setValue('store_id', '')
            }}
          />
        )}
      </PickerField>
      <PickerField form={form} name="system_id" label="体系" required>
        {(field) => (
          <SystemPicker
            departmentId={departmentId || undefined}
            value={field.value}
            onChange={(v) => {
              field.onChange(v)
              form.setValue('store_id', '')
            }}
          />
        )}
      </PickerField>
      <PickerField form={form} name="store_id" label="门店" required>
        {(field) => (
          <StorePicker
            systemId={systemId || undefined}
            value={field.value}
            onChange={field.onChange}
          />
        )}
      </PickerField>
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
      <FormSelect
        control={form.control}
        name="deal_status"
        label="成交状态"
        required
        options={toOptions(DEAL_STATUS_LABELS)}
      />
    </div>
  )
}

/** 第 2 步:内容明细行(动态增删;single 用于编辑单条记录)。 */
export function LinesSection({ form, single = false }: { form: SalesForm; single?: boolean }) {
  const { fields, append, remove } = useFieldArray({ control: form.control, name: 'lines' })
  const { data: categories } = useQuery(activeCategoriesOptions)
  const requiresMap = new Map((categories ?? []).map((c) => [c.id, c.requires_operation_count]))
  const lines = form.watch('lines')

  return (
    <div className="flex flex-col gap-3">
      {fields.map((item, index) => {
        const categoryId = lines[index]?.content_category_id
        const requiresCount = categoryId ? (requiresMap.get(categoryId) ?? false) : false
        return (
          <Card key={item.id}>
            <CardHeader className="flex-row items-center justify-between py-3">
              <CardTitle className="text-sm">明细 {index + 1}</CardTitle>
              {!single && fields.length > 1 && (
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  onClick={() => remove(index)}
                  aria-label={`删除明细 ${index + 1}`}
                >
                  <Trash2 />
                </Button>
              )}
            </CardHeader>
            <CardContent className="grid gap-4 md:grid-cols-3">
              <Controller
                control={form.control}
                name={`lines.${index}.content_category_id`}
                render={({ field, fieldState }) => (
                  <Field data-invalid={fieldState.invalid || undefined}>
                    <FieldLabel>
                      内容类型<span className="text-destructive">*</span>
                    </FieldLabel>
                    <CategoryPicker
                      value={field.value || undefined}
                      onChange={(v) => {
                        field.onChange(v ?? '')
                        // 切换类别时清空次数,避免残留
                        form.setValue(`lines.${index}.operation_total_count`, undefined)
                      }}
                    />
                    {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
                  </Field>
                )}
              />
              <FormMoney control={form.control} name={`lines.${index}.paid_amount`} label="已收金额" required />
              <FormMoney control={form.control} name={`lines.${index}.unpaid_amount`} label="未收金额" required />
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
            </CardContent>
          </Card>
        )
      })}
      {!single && (
        <Button
          type="button"
          variant="outline"
          onClick={() => append({ content_category_id: '', paid_amount: '', unpaid_amount: '' })}
        >
          <Plus />
          添加明细
        </Button>
      )}
    </div>
  )
}

/** 第 3 步:协作与人员。 */
export function CollaborationSection({ form }: { form: SalesForm }) {
  const collaborationType = form.watch('collaboration_type')
  const isExpert = collaborationType === 'expert_consultation'

  return (
    <div className="grid gap-4 md:grid-cols-2">
      <FormSelect
        control={form.control}
        name="collaboration_type"
        label="协作类型"
        required
        options={toOptions(COLLABORATION_TYPE_LABELS)}
      />
      <PickerField form={form} name="handler_user_id" label="处理人" required>
        {(field) => <UserPicker value={field.value} onChange={field.onChange} />}
      </PickerField>
      {isExpert && (
        <>
          <PickerField form={form} name="expert_user_id" label="专家" required>
            {(field) => <UserPicker value={field.value} onChange={field.onChange} placeholder="选择专家" />}
          </PickerField>
          <PickerField form={form} name="expert_department_id" label="专家部门" required>
            {(field) => <DepartmentPicker value={field.value} onChange={field.onChange} placeholder="选择专家部门" />}
          </PickerField>
        </>
      )}
      <PickerField form={form} name="consultant_user_id" label="咨询师">
        {(field) => <UserPicker value={field.value} onChange={field.onChange} placeholder="选择咨询师(选填)" />}
      </PickerField>
      <PickerField form={form} name="consultant_department_id" label="咨询师部门">
        {(field) => (
          <DepartmentPicker value={field.value} onChange={field.onChange} placeholder="选择咨询师部门(选填)" />
        )}
      </PickerField>
      <PickerField form={form} name="doctor_user_id" label="医生">
        {(field) => <UserPicker value={field.value} onChange={field.onChange} placeholder="选择医生(选填)" />}
      </PickerField>
    </div>
  )
}
