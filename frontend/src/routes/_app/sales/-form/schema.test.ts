import { describe, expect, it } from 'vitest'
import {
  assembleSaleRequest,
  assembleServiceRequest,
  ratioToBasisPoints,
  salesFormSchema,
  SALES_FORM_DEFAULTS,
  type SalesFormValues,
} from './schema'

function saleValues(overrides: Partial<SalesFormValues> = {}): SalesFormValues {
  return {
    ...SALES_FORM_DEFAULTS,
    customer_id: 'c1',
    record_date: '2026-07-09',
    handler_user_id: 'u1',
    lines: [
      {
        product_id: 'p1',
        item_name: '项目A',
        receivable_amount: '300.00',
        requires_operation_count: false,
      },
    ],
    payment: {
      paid_amount: '100.00',
      paid_at: '2026-07-09T10:00',
      allocations: [{ guide_user_id: 'g1', allocation_ratio: '100' }],
      remark: '',
    },
    ...overrides,
  }
}

function serviceValues(overrides: Partial<SalesFormValues> = {}): SalesFormValues {
  return saleValues({
    record_type: 'service',
    lines: [
      {
        product_id: 'p1',
        item_name: '服务A',
        receivable_amount: '0.00',
        requires_operation_count: false,
      },
    ],
    // 服务模式付款区不参与校验,允许留空
    payment: { paid_amount: '', paid_at: '', allocations: [], remark: '' },
    ...overrides,
  })
}

describe('ratioToBasisPoints', () => {
  it('整数与两位小数', () => {
    expect(ratioToBasisPoints('100')).toBe(10000)
    expect(ratioToBasisPoints('33.33')).toBe(3333)
    expect(ratioToBasisPoints('0.01')).toBe(1)
    expect(ratioToBasisPoints('60.5')).toBe(6050)
  })
  it('非法输入返回 null', () => {
    expect(ratioToBasisPoints('')).toBeNull()
    expect(ratioToBasisPoints('1.234')).toBeNull()
    expect(ratioToBasisPoints('abc')).toBeNull()
  })
})

describe('salesFormSchema · sale', () => {
  it('合法销售通过', () => {
    expect(salesFormSchema.safeParse(saleValues()).success).toBe(true)
  })

  it('比例合计 ≠ 100 失败', () => {
    const result = salesFormSchema.safeParse(
      saleValues({
        payment: {
          paid_amount: '100.00',
          paid_at: '2026-07-09T10:00',
          allocations: [
            { guide_user_id: 'g1', allocation_ratio: '60' },
            { guide_user_id: 'g2', allocation_ratio: '30' },
          ],
          remark: '',
        },
      }),
    )
    expect(result.success).toBe(false)
    expect(
      result.error?.issues.some((i) => i.path.join('.') === 'payment.allocations'),
    ).toBe(true)
  })

  it('重复导购失败', () => {
    const result = salesFormSchema.safeParse(
      saleValues({
        payment: {
          paid_amount: '100.00',
          paid_at: '2026-07-09T10:00',
          allocations: [
            { guide_user_id: 'g1', allocation_ratio: '50' },
            { guide_user_id: 'g1', allocation_ratio: '50' },
          ],
          remark: '',
        },
      }),
    )
    expect(result.success).toBe(false)
  })

  it('首款超过应收合计失败', () => {
    const result = salesFormSchema.safeParse(
      saleValues({
        payment: {
          paid_amount: '301.00',
          paid_at: '2026-07-09T10:00',
          allocations: [{ guide_user_id: 'g1', allocation_ratio: '100' }],
          remark: '',
        },
      }),
    )
    expect(result.success).toBe(false)
    expect(result.error?.issues.some((i) => i.path.join('.') === 'payment.paid_amount')).toBe(true)
  })

  it('应收全 0 失败', () => {
    const result = salesFormSchema.safeParse(
      saleValues({
        lines: [
          {
            product_id: 'p1',
            item_name: '项目A',
            receivable_amount: '0.00',
            requires_operation_count: false,
          },
        ],
      }),
    )
    expect(result.success).toBe(false)
  })

  it('需要次数的产品缺次数失败', () => {
    const result = salesFormSchema.safeParse(
      saleValues({
        lines: [
          {
            product_id: 'p1',
            item_name: '项目A',
            receivable_amount: '300.00',
            requires_operation_count: true,
          },
        ],
      }),
    )
    expect(result.success).toBe(false)
    expect(
      result.error?.issues.some((i) => i.path.join('.') === 'lines.0.operation_total_count'),
    ).toBe(true)
  })

  it('不需要次数的产品带次数失败', () => {
    const result = salesFormSchema.safeParse(
      saleValues({
        lines: [
          {
            product_id: 'p1',
            item_name: '项目A',
            receivable_amount: '300.00',
            operation_total_count: 5,
            requires_operation_count: false,
          },
        ],
      }),
    )
    expect(result.success).toBe(false)
  })

  it('金额格式 1.234 失败', () => {
    const result = salesFormSchema.safeParse(
      saleValues({
        lines: [
          {
            product_id: 'p1',
            item_name: '项目A',
            receivable_amount: '1.234',
            requires_operation_count: false,
          },
        ],
      }),
    )
    expect(result.success).toBe(false)
  })
})

describe('salesFormSchema · service', () => {
  it('合法服务通过(付款区留空)', () => {
    expect(salesFormSchema.safeParse(serviceValues()).success).toBe(true)
  })

  it('服务行金额非 0 失败', () => {
    const result = salesFormSchema.safeParse(
      serviceValues({
        lines: [
          {
            product_id: 'p1',
            item_name: '服务A',
            receivable_amount: '1.00',
            requires_operation_count: false,
          },
        ],
      }),
    )
    expect(result.success).toBe(false)
  })

  it('服务行带次数失败', () => {
    const result = salesFormSchema.safeParse(
      serviceValues({
        lines: [
          {
            product_id: 'p1',
            item_name: '服务A',
            receivable_amount: '0.00',
            operation_total_count: 3,
            requires_operation_count: true,
          },
        ],
      }),
    )
    expect(result.success).toBe(false)
  })
})

describe('assembleSaleRequest', () => {
  it('deny_unknown_fields 守卫:输出键精确匹配契约', () => {
    const request = assembleSaleRequest(
      saleValues({
        lines: [
          {
            product_id: 'p1',
            item_name: '项目A',
            receivable_amount: '300.00',
            operation_total_count: 5,
            requires_operation_count: true,
          },
        ],
      }),
    )
    expect(Object.keys(request).sort()).toEqual(
      [
        'customer_id',
        'record_date',
        'customer_type',
        'deal_type',
        'handler_user_id',
        'expert_user_id',
        'consultant_user_id',
        'doctor_user_id',
        'remark',
        'lines',
        'payment',
      ].sort(),
    )
    expect(Object.keys(request.lines[0]!).sort()).toEqual(
      ['product_id', 'item_name', 'receivable_amount', 'operation_total_count', 'remark'].sort(),
    )
    expect(Object.keys(request.payment).sort()).toEqual(
      ['paid_amount', 'paid_at', 'allocations', 'remark'].sort(),
    )
    expect(Object.keys(request.payment.allocations[0]!).sort()).toEqual(
      ['guide_user_id', 'allocation_ratio'].sort(),
    )
  })

  it('requires 行发送次数,非 requires 行次数为 null', () => {
    const request = assembleSaleRequest(
      saleValues({
        lines: [
          {
            product_id: 'p1',
            item_name: 'A',
            receivable_amount: '100.00',
            operation_total_count: 5,
            requires_operation_count: true,
          },
          {
            product_id: 'p2',
            item_name: 'B',
            receivable_amount: '200.00',
            requires_operation_count: false,
          },
        ],
      }),
    )
    expect(request.lines[0]?.operation_total_count).toBe(5)
    expect(request.lines[1]?.operation_total_count).toBeNull()
  })

  it('paid_at 转为 ISO;空可选人员为 null', () => {
    const request = assembleSaleRequest(saleValues())
    expect(request.payment.paid_at).toMatch(/Z$/)
    expect(new Date(request.payment.paid_at).getTime()).toBe(
      new Date('2026-07-09T10:00').getTime(),
    )
    expect(request.expert_user_id).toBeNull()
    expect(request.consultant_user_id).toBeNull()
    expect(request.doctor_user_id).toBeNull()
  })
})

describe('assembleServiceRequest', () => {
  it('无 payment 键,行金额硬编码 0.00,无次数键', () => {
    const request = assembleServiceRequest(
      serviceValues({
        lines: [
          {
            product_id: 'p1',
            item_name: '服务A',
            // 即便 UI 出错留下非 0 值,组装器也强制 0.00
            receivable_amount: '5.00',
            requires_operation_count: false,
          },
        ],
      }),
    )
    expect('payment' in request).toBe(false)
    expect(request.lines[0]?.receivable_amount).toBe('0.00')
    expect('operation_total_count' in request.lines[0]!).toBe(false)
    expect(Object.keys(request).sort()).toEqual(
      [
        'customer_id',
        'record_date',
        'customer_type',
        'deal_type',
        'handler_user_id',
        'expert_user_id',
        'consultant_user_id',
        'doctor_user_id',
        'remark',
        'lines',
      ].sort(),
    )
  })
})
