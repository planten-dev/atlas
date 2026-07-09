import { z } from 'zod'
import type { CreateSalesRecordRequest } from '@/hooks/useSales'

const AMOUNT_MESSAGE = '金额格式:最多 10 位整数 + 2 位小数'
const amountSchema = z
  .string()
  .min(1, '请填写金额')
  .regex(/^\d{1,10}(\.\d{1,2})?$/, AMOUNT_MESSAGE)

/** 明细行(设计 §9):内容类型 + 已收/未收金额 + 可操作次数(条件必填)。 */
export const salesLineSchema = z.object({
  content_category_id: z.string().min(1, '请选择内容类型'),
  paid_amount: amountSchema,
  unpaid_amount: amountSchema,
  operation_total_count: z.number().int('必须为整数').min(1, '至少 1 次').optional(),
})

/** 公共区:客户/归属(体系→门店两级)/类型/协作/人员。 */
export const salesCommonSchema = z.object({
  customer_id: z.string().min(1, '请选择客户'),
  sale_date: z.string().min(1, '请选择成交日期'),
  system_id: z.string().min(1, '请选择体系'),
  store_id: z.string().min(1, '请选择门店'),
  customer_type: z.enum(['new', 'returning'], '请选择客户类型'),
  deal_type: z.enum(['non_salon', 'salon'], '请选择成交类型'),
  deal_status: z.enum(['closed', 'not_closed'], '请选择成交状态'),
  collaboration_type: z.enum(['expert_consultation', 'self_sale'], '请选择协作类型'),
  expert_user_id: z.string().optional(),
  consultant_user_id: z.string().optional(),
  doctor_user_id: z.string().optional(),
  handler_user_id: z.string().min(1, '请选择处理人'),
})

export type SalesLineValues = z.infer<typeof salesLineSchema>
export type SalesCommonValues = z.infer<typeof salesCommonSchema>

export interface SalesFormValues extends SalesCommonValues {
  lines: SalesLineValues[]
}

/**
 * 表单级 schema 工厂:注入"类别 → 是否需要次数"表以校验 operation_total_count 条件必填;
 * 协作类型规则与后端对齐:专家诊必填专家,自销强制清空。
 */
export function buildSalesFormSchema(requiresCountByCategory: ReadonlyMap<string, boolean>) {
  return salesCommonSchema
    .extend({
      lines: z.array(salesLineSchema).min(1, '至少一条明细'),
    })
    .superRefine((values, ctx) => {
      if (values.collaboration_type === 'expert_consultation' && !values.expert_user_id) {
        ctx.addIssue({ code: 'custom', path: ['expert_user_id'], message: '专家诊必须选择专家' })
      }
      values.lines.forEach((line, index) => {
        const requires = requiresCountByCategory.get(line.content_category_id) ?? false
        if (requires && (line.operation_total_count === undefined || line.operation_total_count === null)) {
          ctx.addIssue({
            code: 'custom',
            path: ['lines', index, 'operation_total_count'],
            message: '该类别需要填写可操作次数',
          })
        }
      })
    })
}

/** 公共区 × 明细行 → create-batch 的 records[](设计 §9)。 */
export function assembleRecords(
  values: SalesFormValues,
  requiresCountByCategory: ReadonlyMap<string, boolean>,
): CreateSalesRecordRequest[] {
  const isExpert = values.collaboration_type === 'expert_consultation'
  return values.lines.map((line) => ({
    customer_id: values.customer_id,
    sale_date: values.sale_date,
    deal_status: values.deal_status,
    customer_type: values.customer_type,
    deal_type: values.deal_type,
    content_category_id: line.content_category_id,
    handler_user_id: values.handler_user_id,
    paid_amount: line.paid_amount,
    unpaid_amount: line.unpaid_amount,
    system_id: values.system_id,
    store_id: values.store_id,
    collaboration_type: values.collaboration_type,
    // 自销强制清空专家字段(与后端校验对齐)
    expert_user_id: isExpert ? (values.expert_user_id ?? null) : null,
    consultant_user_id: values.consultant_user_id || null,
    doctor_user_id: values.doctor_user_id || null,
    operation_total_count: requiresCountByCategory.get(line.content_category_id)
      ? (line.operation_total_count ?? null)
      : null,
  }))
}

export const SALES_FORM_DEFAULTS: SalesFormValues = {
  customer_id: '',
  sale_date: '',
  system_id: '',
  store_id: '',
  customer_type: 'new',
  deal_type: 'non_salon',
  deal_status: 'closed',
  collaboration_type: 'self_sale',
  expert_user_id: '',
  consultant_user_id: '',
  doctor_user_id: '',
  handler_user_id: '',
  lines: [{ content_category_id: '', paid_amount: '', unpaid_amount: '' }],
}
