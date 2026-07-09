import { z } from 'zod'
import { compareAmounts, sumAmounts } from '@/lib/money'
import type { CreateSaleRecordRequest, CreateServiceRecordRequest } from '@/hooks/useSales'

const AMOUNT_MESSAGE = '金额格式:最多 10 位整数 + 2 位小数'
const amountSchema = z
  .string()
  .min(1, '请填写金额')
  .regex(/^\d{1,10}(\.\d{1,2})?$/, AMOUNT_MESSAGE)

const RATIO_MESSAGE = '比例 0.01–100,最多 2 位小数'
const ratioSchema = z
  .string()
  .min(1, '请填写比例')
  .regex(/^\d{1,3}(\.\d{1,2})?$/, RATIO_MESSAGE)

/** 比例 → 百分之一分(×100 整数),避免浮点;非法输入返回 null。 */
export function ratioToBasisPoints(ratio: string): number | null {
  const match = /^(\d{1,3})(?:\.(\d{1,2}))?$/.exec(ratio)
  if (!match) return null
  const whole = Number(match[1])
  const frac = Number((match[2] ?? '').padEnd(2, '0') || '0')
  return whole * 100 + frac
}

export const allocationSchema = z.object({
  guide_user_id: z.string().min(1, '请选择导购'),
  allocation_ratio: ratioSchema,
})

/** 明细行:产品 + 项目名称 + 应收金额 + 可操作次数(按产品类别条件必填)。 */
export const salesLineSchema = z.object({
  product_id: z.string().min(1, '请选择产品'),
  item_name: z.string().min(1, '请填写项目名称').max(128, '项目名称最长 128 字'),
  receivable_amount: amountSchema,
  operation_total_count: z.number().int('必须为整数').min(1, '至少 1 次').optional(),
  remark: z.string().max(2000, '备注最长 2000 字').optional(),
  // 选产品时快照的内部字段,提交时剔除(后端 deny_unknown_fields)
  requires_operation_count: z.boolean(),
})

export const salesFormSchema = z
  .object({
    record_type: z.enum(['sale', 'service'], '请选择记录类型'),
    customer_id: z.string().min(1, '请选择客户'),
    record_date: z.string().min(1, '请选择成交日期'),
    customer_type: z.enum(['new', 'returning'], '请选择客户类型'),
    deal_type: z.enum(['non_salon', 'salon'], '请选择成交类型'),
    handler_user_id: z.string().min(1, '请选择处理人'),
    expert_user_id: z.string().optional(),
    consultant_user_id: z.string().optional(),
    doctor_user_id: z.string().optional(),
    remark: z.string().max(2000, '备注最长 2000 字').optional(),
    lines: z.array(salesLineSchema).min(1, '至少一条明细'),
    payment: z.object({
      paid_amount: z.string(),
      paid_at: z.string(),
      allocations: z.array(allocationSchema),
      remark: z.string().max(2000, '备注最长 2000 字').optional(),
    }),
  })
  .superRefine((values, ctx) => {
    if (values.record_type === 'service') {
      // 服务记录:行金额恒 0(UI 锁定,此处兜底)、禁填次数、无付款校验
      values.lines.forEach((line, index) => {
        if (line.receivable_amount !== '' && compareAmounts(line.receivable_amount || '0', '0') !== 0) {
          ctx.addIssue({
            code: 'custom',
            path: ['lines', index, 'receivable_amount'],
            message: '服务记录金额必须为 0',
          })
        }
        if (line.operation_total_count !== undefined) {
          ctx.addIssue({
            code: 'custom',
            path: ['lines', index, 'operation_total_count'],
            message: '服务记录不可填写次数',
          })
        }
      })
      return
    }

    // 销售记录:行金额/次数规则
    values.lines.forEach((line, index) => {
      if (line.requires_operation_count && line.operation_total_count === undefined) {
        ctx.addIssue({
          code: 'custom',
          path: ['lines', index, 'operation_total_count'],
          message: '该产品类别需要填写可操作次数',
        })
      }
      if (!line.requires_operation_count && line.operation_total_count !== undefined) {
        ctx.addIssue({
          code: 'custom',
          path: ['lines', index, 'operation_total_count'],
          message: '该产品类别不可填写次数',
        })
      }
    })

    const amounts = values.lines
      .map((line) => line.receivable_amount)
      .filter((amount) => /^\d{1,10}(\.\d{1,2})?$/.test(amount))
    if (amounts.length === values.lines.length) {
      const total = sumAmounts(amounts)
      if (compareAmounts(total, '0') <= 0) {
        ctx.addIssue({
          code: 'custom',
          path: ['lines', 0, 'receivable_amount'],
          message: '销售记录应收合计必须大于 0',
        })
      }

      // 首款校验
      const paid = values.payment.paid_amount
      if (!paid || !/^\d{1,10}(\.\d{1,2})?$/.test(paid)) {
        ctx.addIssue({
          code: 'custom',
          path: ['payment', 'paid_amount'],
          message: paid ? AMOUNT_MESSAGE : '请填写首款金额',
        })
      } else {
        if (compareAmounts(paid, '0') <= 0) {
          ctx.addIssue({
            code: 'custom',
            path: ['payment', 'paid_amount'],
            message: '首款金额必须大于 0',
          })
        } else if (compareAmounts(paid, total) > 0) {
          ctx.addIssue({
            code: 'custom',
            path: ['payment', 'paid_amount'],
            message: '首款金额不能超过应收合计',
          })
        }
      }
    }

    if (!values.payment.paid_at) {
      ctx.addIssue({ code: 'custom', path: ['payment', 'paid_at'], message: '请选择支付时间' })
    }
    if (values.payment.allocations.length === 0) {
      ctx.addIssue({
        code: 'custom',
        path: ['payment', 'allocations'],
        message: '至少一条业绩分配',
      })
    } else {
      const points = values.payment.allocations.map((a) => ratioToBasisPoints(a.allocation_ratio))
      if (points.every((p): p is number => p !== null)) {
        points.forEach((p, index) => {
          if (p <= 0 || p > 10000) {
            ctx.addIssue({
              code: 'custom',
              path: ['payment', 'allocations', index, 'allocation_ratio'],
              message: RATIO_MESSAGE,
            })
          }
        })
        const sum = points.reduce((acc, p) => acc + p, 0)
        if (sum !== 10000) {
          ctx.addIssue({
            code: 'custom',
            path: ['payment', 'allocations'],
            message: '分配比例合计必须等于 100%',
          })
        }
      }
      const guides = values.payment.allocations.map((a) => a.guide_user_id).filter(Boolean)
      if (new Set(guides).size !== guides.length) {
        ctx.addIssue({
          code: 'custom',
          path: ['payment', 'allocations'],
          message: '同一导购不能重复分配',
        })
      }
    }
  })

export type SalesLineValues = z.infer<typeof salesLineSchema>
export type SalesFormValues = z.infer<typeof salesFormSchema>

/**
 * 提交体逐键构造:后端 deny_unknown_fields,内部字段(record_type/requires_operation_count)
 * 与多余键绝不外泄。
 */
export function assembleSaleRequest(values: SalesFormValues): CreateSaleRecordRequest {
  return {
    customer_id: values.customer_id,
    record_date: values.record_date,
    customer_type: values.customer_type,
    deal_type: values.deal_type,
    handler_user_id: values.handler_user_id,
    expert_user_id: values.expert_user_id || null,
    consultant_user_id: values.consultant_user_id || null,
    doctor_user_id: values.doctor_user_id || null,
    remark: values.remark || null,
    lines: values.lines.map((line) => ({
      product_id: line.product_id,
      item_name: line.item_name,
      receivable_amount: line.receivable_amount,
      operation_total_count: line.requires_operation_count
        ? (line.operation_total_count ?? null)
        : null,
      remark: line.remark || null,
    })),
    payment: {
      paid_amount: values.payment.paid_amount,
      paid_at: new Date(values.payment.paid_at).toISOString(),
      allocations: values.payment.allocations.map((allocation) => ({
        guide_user_id: allocation.guide_user_id,
        allocation_ratio: allocation.allocation_ratio,
      })),
      remark: values.payment.remark || null,
    },
  }
}

/** 服务记录:无 payment 键,行金额硬编码 "0.00",次数不发送。 */
export function assembleServiceRequest(values: SalesFormValues): CreateServiceRecordRequest {
  return {
    customer_id: values.customer_id,
    record_date: values.record_date,
    customer_type: values.customer_type,
    deal_type: values.deal_type,
    handler_user_id: values.handler_user_id,
    expert_user_id: values.expert_user_id || null,
    consultant_user_id: values.consultant_user_id || null,
    doctor_user_id: values.doctor_user_id || null,
    remark: values.remark || null,
    lines: values.lines.map((line) => ({
      product_id: line.product_id,
      item_name: line.item_name,
      receivable_amount: '0.00',
      remark: line.remark || null,
    })),
  }
}

export const EMPTY_LINE: SalesLineValues = {
  product_id: '',
  item_name: '',
  receivable_amount: '',
  requires_operation_count: false,
}

export const SALES_FORM_DEFAULTS: SalesFormValues = {
  record_type: 'sale',
  customer_id: '',
  record_date: '',
  customer_type: 'new',
  deal_type: 'non_salon',
  handler_user_id: '',
  expert_user_id: '',
  consultant_user_id: '',
  doctor_user_id: '',
  remark: '',
  lines: [{ ...EMPTY_LINE }],
  payment: {
    paid_amount: '',
    paid_at: '',
    allocations: [{ guide_user_id: '', allocation_ratio: '100' }],
    remark: '',
  },
}
