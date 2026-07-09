import { describe, expect, it } from 'vitest'
import {
  assembleRecords,
  buildSalesFormSchema,
  SALES_FORM_DEFAULTS,
  type SalesFormValues,
} from './schema'

const CATEGORY_NORMAL = 'cat-normal'
const CATEGORY_COUNTED = 'cat-counted'
const requiresMap = new Map([
  [CATEGORY_NORMAL, false],
  [CATEGORY_COUNTED, true],
])

function validValues(overrides: Partial<SalesFormValues> = {}): SalesFormValues {
  return {
    ...SALES_FORM_DEFAULTS,
    customer_id: 'c1',
    sale_date: '2026-07-08',
    system_id: 'sys1',
    store_id: 'st1',
    handler_user_id: 'u1',
    lines: [{ content_category_id: CATEGORY_NORMAL, paid_amount: '100.00', unpaid_amount: '0' }],
    ...overrides,
  }
}

describe('buildSalesFormSchema', () => {
  const schema = buildSalesFormSchema(requiresMap)

  it('合法数据通过', () => {
    expect(schema.safeParse(validValues()).success).toBe(true)
  })

  it('专家诊必填专家', () => {
    const result = schema.safeParse(
      validValues({ collaboration_type: 'expert_consultation' }),
    )
    expect(result.success).toBe(false)
    const paths = result.success ? [] : result.error.issues.map((i) => i.path.join('.'))
    expect(paths).toContain('expert_user_id')
  })

  it('专家诊填齐后通过', () => {
    const result = schema.safeParse(
      validValues({
        collaboration_type: 'expert_consultation',
        expert_user_id: 'e1',
      }),
    )
    expect(result.success).toBe(true)
  })

  it('需要次数的类别缺 operation_total_count 报错', () => {
    const result = schema.safeParse(
      validValues({
        lines: [{ content_category_id: CATEGORY_COUNTED, paid_amount: '1.00', unpaid_amount: '0' }],
      }),
    )
    expect(result.success).toBe(false)
    const paths = result.success ? [] : result.error.issues.map((i) => i.path.join('.'))
    expect(paths).toContain('lines.0.operation_total_count')
  })

  it('金额格式校验(pattern 对齐后端)', () => {
    const result = schema.safeParse(
      validValues({
        lines: [{ content_category_id: CATEGORY_NORMAL, paid_amount: '1.234', unpaid_amount: '0' }],
      }),
    )
    expect(result.success).toBe(false)
  })

  it('至少一条明细', () => {
    const result = schema.safeParse(validValues({ lines: [] }))
    expect(result.success).toBe(false)
  })
})

describe('assembleRecords', () => {
  it('公共区 × 2 行明细 = 2 条 records', () => {
    const records = assembleRecords(
      validValues({
        lines: [
          { content_category_id: CATEGORY_NORMAL, paid_amount: '100.00', unpaid_amount: '0' },
          {
            content_category_id: CATEGORY_COUNTED,
            paid_amount: '200.00',
            unpaid_amount: '50.00',
            operation_total_count: 10,
          },
        ],
      }),
      requiresMap,
    )
    expect(records).toHaveLength(2)
    expect(records[0]?.customer_id).toBe('c1')
    expect(records[1]?.customer_id).toBe('c1')
    expect(records[0]?.operation_total_count).toBeNull()
    expect(records[1]?.operation_total_count).toBe(10)
  })

  it('自销强制清空专家字段(与后端校验对齐)', () => {
    const records = assembleRecords(
      validValues({
        collaboration_type: 'self_sale',
        expert_user_id: 'e1',
      }),
      requiresMap,
    )
    expect(records[0]?.expert_user_id).toBeNull()
  })

  it('专家诊保留专家字段', () => {
    const records = assembleRecords(
      validValues({
        collaboration_type: 'expert_consultation',
        expert_user_id: 'e1',
      }),
      requiresMap,
    )
    expect(records[0]?.expert_user_id).toBe('e1')
  })

  it('不需要次数的类别忽略残留 operation_total_count', () => {
    const records = assembleRecords(
      validValues({
        lines: [
          {
            content_category_id: CATEGORY_NORMAL,
            paid_amount: '1.00',
            unpaid_amount: '0',
            operation_total_count: 5,
          },
        ],
      }),
      requiresMap,
    )
    expect(records[0]?.operation_total_count).toBeNull()
  })

  it('选填人员空串转 null', () => {
    const records = assembleRecords(validValues(), requiresMap)
    expect(records[0]?.consultant_user_id).toBeNull()
    expect(records[0]?.doctor_user_id).toBeNull()
  })
})
