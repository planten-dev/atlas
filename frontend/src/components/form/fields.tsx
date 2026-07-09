import type { ReactNode } from 'react'
import {
  Controller,
  type Control,
  type FieldPath,
  type FieldValues,
} from 'react-hook-form'
import { Field, FieldError, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Checkbox } from '@/components/ui/checkbox'
import { Switch } from '@/components/ui/switch'
import { formatAmount, isValidAmount } from '@/lib/money'

/**
 * RHF + zod 字段套件(设计 §3 components/form)。
 * 每个字段 = Controller + shadcn Field 原语,错误文案来自 zod。
 */

interface BaseFieldProps<TFieldValues extends FieldValues> {
  control: Control<TFieldValues>
  name: FieldPath<TFieldValues>
  label: string
  required?: boolean
  disabled?: boolean
}

function LabelText({ label, required }: { label: string; required?: boolean }) {
  return (
    <>
      {label}
      {required && <span className="text-destructive">*</span>}
    </>
  )
}

export function FormText<T extends FieldValues>({
  control,
  name,
  label,
  required,
  disabled,
  placeholder,
  type = 'text',
}: BaseFieldProps<T> & { placeholder?: string; type?: string }) {
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field data-invalid={fieldState.invalid || undefined}>
          <FieldLabel htmlFor={name}>
            <LabelText label={label} required={required} />
          </FieldLabel>
          <Input
            id={name}
            type={type}
            placeholder={placeholder}
            disabled={disabled}
            aria-invalid={fieldState.invalid || undefined}
            {...field}
            value={field.value ?? ''}
          />
          {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
        </Field>
      )}
    />
  )
}

export function FormTextarea<T extends FieldValues>({
  control,
  name,
  label,
  required,
  disabled,
  placeholder,
  rows = 3,
}: BaseFieldProps<T> & { placeholder?: string; rows?: number }) {
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field data-invalid={fieldState.invalid || undefined}>
          <FieldLabel htmlFor={name}>
            <LabelText label={label} required={required} />
          </FieldLabel>
          <Textarea
            id={name}
            placeholder={placeholder}
            rows={rows}
            disabled={disabled}
            aria-invalid={fieldState.invalid || undefined}
            {...field}
            value={field.value ?? ''}
          />
          {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
        </Field>
      )}
    />
  )
}

export interface SelectOption {
  value: string
  label: string
}

export function FormSelect<T extends FieldValues>({
  control,
  name,
  label,
  required,
  disabled,
  options,
  placeholder = '请选择',
}: BaseFieldProps<T> & { options: SelectOption[]; placeholder?: string }) {
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field data-invalid={fieldState.invalid || undefined}>
          <FieldLabel>
            <LabelText label={label} required={required} />
          </FieldLabel>
          <Select
            // items 让触发器按选项 label 渲染选中值,而非原始 value
            items={options}
            value={field.value ?? null}
            onValueChange={(value) => field.onChange(value ?? undefined)}
            disabled={disabled}
          >
            <SelectTrigger aria-invalid={fieldState.invalid || undefined} className="w-full">
              <SelectValue placeholder={placeholder} />
            </SelectTrigger>
            <SelectContent>
              {options.map((opt) => (
                <SelectItem key={opt.value} value={opt.value}>
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
        </Field>
      )}
    />
  )
}

/** 金额输入:inputMode=decimal,失焦归一为两位小数字符串。 */
export function FormMoney<T extends FieldValues>({
  control,
  name,
  label,
  required,
  disabled,
  placeholder = '0.00',
}: BaseFieldProps<T> & { placeholder?: string }) {
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field data-invalid={fieldState.invalid || undefined}>
          <FieldLabel htmlFor={name}>
            <LabelText label={label} required={required} />
          </FieldLabel>
          <Input
            id={name}
            inputMode="decimal"
            placeholder={placeholder}
            disabled={disabled}
            aria-invalid={fieldState.invalid || undefined}
            {...field}
            value={field.value ?? ''}
            onBlur={() => {
              const v = field.value as string | undefined
              if (v && isValidAmount(v)) {
                field.onChange(formatAmount(v))
              }
              field.onBlur()
            }}
          />
          {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
        </Field>
      )}
    />
  )
}

/** 日期输入:统一用原生 date input(移动端体验最佳,桌面可用)。 */
export function FormDate<T extends FieldValues>({
  control,
  name,
  label,
  required,
  disabled,
}: BaseFieldProps<T>) {
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field data-invalid={fieldState.invalid || undefined}>
          <FieldLabel htmlFor={name}>
            <LabelText label={label} required={required} />
          </FieldLabel>
          <Input
            id={name}
            type="date"
            disabled={disabled}
            aria-invalid={fieldState.invalid || undefined}
            {...field}
            value={field.value ?? ''}
          />
          {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
        </Field>
      )}
    />
  )
}

export function FormCheckbox<T extends FieldValues>({
  control,
  name,
  label,
  disabled,
  description,
}: Omit<BaseFieldProps<T>, 'required'> & { description?: ReactNode }) {
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field orientation="horizontal" data-invalid={fieldState.invalid || undefined}>
          <Checkbox
            id={name}
            checked={Boolean(field.value)}
            onCheckedChange={(checked) => field.onChange(checked === true)}
            disabled={disabled}
          />
          <FieldLabel htmlFor={name} className="font-normal">
            {label}
            {description && <span className="text-muted-foreground"> {description}</span>}
          </FieldLabel>
          {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
        </Field>
      )}
    />
  )
}

export function FormSwitch<T extends FieldValues>({
  control,
  name,
  label,
  disabled,
}: Omit<BaseFieldProps<T>, 'required'>) {
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field orientation="horizontal" data-invalid={fieldState.invalid || undefined}>
          <FieldLabel htmlFor={name}>{label}</FieldLabel>
          <Switch
            id={name}
            checked={Boolean(field.value)}
            onCheckedChange={(checked) => field.onChange(checked)}
            disabled={disabled}
          />
          {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
        </Field>
      )}
    />
  )
}
