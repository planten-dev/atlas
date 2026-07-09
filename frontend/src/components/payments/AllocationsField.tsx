import { Controller, useFieldArray, type Control, type FieldPath, type FieldValues } from 'react-hook-form'
import { Plus, Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Field, FieldError, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { UserPicker } from '@/components/pickers/UserPicker'
import { ratioToBasisPoints } from '@/routes/_app/sales/-form/schema'

interface AllocationRow {
  guide_user_id: string
  allocation_ratio: string
}

/**
 * 业绩分配编辑器(销售录入首款 + 回款 Dialog 共用):
 * 导购 + 比例(合计须 100%,实时指示);金额按比例服务端分配,尾差归入最后一条。
 */
export function AllocationsField<T extends FieldValues>({
  control,
  name,
  rows,
  error,
}: {
  control: Control<T>
  /** 指向 AllocationRow[] 的字段路径,如 'payment.allocations' */
  name: FieldPath<T>
  /** watch 得到的当前行,用于合计指示 */
  rows: AllocationRow[]
  /** 数组级错误信息(比例合计/重复导购) */
  error?: string
}) {
  const { fields, append, remove } = useFieldArray({
    control,
    // useFieldArray 泛型不接受任意 FieldPath,统一在调用侧保证路径指向数组
    name: name as never,
  })

  const points = rows.map((row) => ratioToBasisPoints(row.allocation_ratio))
  const sum = points.every((p): p is number => p !== null)
    ? points.reduce((acc, p) => acc + p, 0)
    : null
  const sumText = sum === null ? '—' : `${(sum / 100).toFixed(2)}%`

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <FieldLabel>
          业绩分配<span className="text-destructive">*</span>
        </FieldLabel>
        <span
          className={`text-xs tabular-nums ${sum === 10000 ? 'text-muted-foreground' : 'text-destructive'}`}
        >
          合计 {sumText} / 100%
        </span>
      </div>
      {fields.map((item, index) => (
        <div key={item.id} className="flex items-start gap-2">
          <Controller
            control={control}
            name={`${name}.${index}.guide_user_id` as FieldPath<T>}
            render={({ field, fieldState }) => (
              <Field className="flex-1" data-invalid={fieldState.invalid || undefined}>
                <UserPicker
                  value={(field.value as string) || undefined}
                  onChange={(v) => field.onChange(v ?? '')}
                  placeholder="选择导购"
                />
                {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
              </Field>
            )}
          />
          <Controller
            control={control}
            name={`${name}.${index}.allocation_ratio` as FieldPath<T>}
            render={({ field, fieldState }) => (
              <Field className="w-28" data-invalid={fieldState.invalid || undefined}>
                <div className="relative">
                  <Input
                    inputMode="decimal"
                    value={(field.value as string) ?? ''}
                    onChange={(e) => field.onChange(e.target.value)}
                    aria-invalid={fieldState.invalid || undefined}
                    className="pr-7"
                  />
                  <span className="pointer-events-none absolute inset-y-0 right-2 flex items-center text-sm text-muted-foreground">
                    %
                  </span>
                </div>
                {fieldState.error && <FieldError>{fieldState.error.message}</FieldError>}
              </Field>
            )}
          />
          {fields.length > 1 && (
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="mt-0.5 text-muted-foreground hover:text-destructive"
              onClick={() => remove(index)}
              aria-label={`删除分配 ${index + 1}`}
            >
              <Trash2 />
            </Button>
          )}
        </div>
      ))}
      {error && <FieldError>{error}</FieldError>}
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => append({ guide_user_id: '', allocation_ratio: '' } as never)}
      >
        <Plus />
        添加分配
      </Button>
      <p className="text-xs text-muted-foreground">金额按比例分配,尾差由系统归入最后一条。</p>
    </div>
  )
}
