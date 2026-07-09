import { useForm, Controller, type UseFormReturn } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Field, FieldError, FieldLabel } from '@/components/ui/field'
import { FormText, FormTextarea } from '@/components/form/fields'
import { DepartmentPicker } from '@/components/pickers/DepartmentPicker'
import { SystemPicker } from '@/components/pickers/SystemPicker'
import { StorePicker } from '@/components/pickers/StorePicker'

export const customerFormSchema = z.object({
  name: z.string().min(1, '请填写客户姓名').max(64, '姓名过长'),
  department_id: z.string().min(1, '请选择部门'),
  system_id: z.string().min(1, '请选择体系'),
  store_id: z.string().min(1, '请选择门店'),
  remark: z.string().max(2000, '备注过长').optional(),
})

export type CustomerFormValues = z.infer<typeof customerFormSchema>

export function useCustomerForm(defaults?: Partial<CustomerFormValues>) {
  return useForm<CustomerFormValues>({
    resolver: zodResolver(customerFormSchema),
    defaultValues: {
      name: '',
      department_id: '',
      system_id: '',
      store_id: '',
      remark: '',
      ...defaults,
    },
  })
}

/** 客户表单字段(新建/编辑/内嵌新建共用):部门→体系→门店三级联动。 */
export function CustomerFormFields({ form }: { form: UseFormReturn<CustomerFormValues> }) {
  const departmentId = form.watch('department_id')
  const systemId = form.watch('system_id')

  return (
    <div className="flex flex-col gap-4">
      <FormText control={form.control} name="name" label="客户姓名" required />

      <Controller
        control={form.control}
        name="department_id"
        render={({ field, fieldState }) => (
          <Field data-invalid={fieldState.invalid || undefined}>
            <FieldLabel>
              部门<span className="text-destructive">*</span>
            </FieldLabel>
            <DepartmentPicker
              value={field.value || undefined}
              onChange={(value) => {
                field.onChange(value ?? '')
                form.setValue('system_id', '')
                form.setValue('store_id', '')
              }}
            />
            {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
          </Field>
        )}
      />

      <Controller
        control={form.control}
        name="system_id"
        render={({ field, fieldState }) => (
          <Field data-invalid={fieldState.invalid || undefined}>
            <FieldLabel>
              体系<span className="text-destructive">*</span>
            </FieldLabel>
            <SystemPicker
              departmentId={departmentId || undefined}
              value={field.value || undefined}
              onChange={(value) => {
                field.onChange(value ?? '')
                form.setValue('store_id', '')
              }}
            />
            {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
          </Field>
        )}
      />

      <Controller
        control={form.control}
        name="store_id"
        render={({ field, fieldState }) => (
          <Field data-invalid={fieldState.invalid || undefined}>
            <FieldLabel>
              门店<span className="text-destructive">*</span>
            </FieldLabel>
            <StorePicker
              systemId={systemId || undefined}
              value={field.value || undefined}
              onChange={(value) => field.onChange(value ?? '')}
            />
            {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
          </Field>
        )}
      />

      <FormTextarea control={form.control} name="remark" label="备注" />

      <div className="rounded-md border border-dashed p-3 text-sm text-muted-foreground">
        附件功能建设中(等待文件服务上线)
      </div>
    </div>
  )
}
