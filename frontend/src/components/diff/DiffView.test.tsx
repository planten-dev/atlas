import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { computeDiffRows, DiffView } from '@/components/diff/DiffView'

describe('computeDiffRows', () => {
  it('update:标记变化字段,保留未变字段', () => {
    const rows = computeDiffRows(
      { name: '旧名', unit_price: '10.00', remark: 'r' },
      { name: '新名', unit_price: '10.00', remark: 'r' },
    )
    const nameRow = rows.find((r) => r.key === 'name')
    const priceRow = rows.find((r) => r.key === 'unit_price')
    expect(nameRow?.changed).toBe(true)
    expect(nameRow?.oldText).toBe('旧名')
    expect(nameRow?.newText).toBe('新名')
    expect(priceRow?.changed).toBe(false)
  })

  it('create(old=null):全部字段呈现为新增', () => {
    const rows = computeDiffRows(null, { name: 'A', status: 'active' })
    expect(rows).toHaveLength(2)
    expect(rows.every((r) => r.oldText === '—')).toBe(true)
    expect(rows.every((r) => r.changed)).toBe(true)
  })

  it('delete(new=null):全部字段呈现为移除', () => {
    const rows = computeDiffRows({ name: 'A' }, null)
    expect(rows[0]?.newText).toBe('—')
    expect(rows[0]?.changed).toBe(true)
  })

  it('金额字段归一两位小数展示', () => {
    const rows = computeDiffRows({ unit_price: '5' }, { unit_price: '6.5' })
    expect(rows[0]?.oldText).toBe('5.00')
    expect(rows[0]?.newText).toBe('6.50')
  })

  it('键并集:新增/删除的字段都出现', () => {
    const rows = computeDiffRows({ a: 1 }, { b: 2 })
    expect(rows.map((r) => r.key).sort()).toEqual(['a', 'b'])
  })

  it('隐藏 id/created_at/updated_at', () => {
    const rows = computeDiffRows(
      { id: 'x', created_at: 't', name: 'A' },
      { id: 'x', created_at: 't', name: 'B' },
    )
    expect(rows.map((r) => r.key)).toEqual(['name'])
  })

  it('布尔值渲染为 是/否', () => {
    const rows = computeDiffRows(
      { requires_operation_count: false },
      { requires_operation_count: true },
    )
    expect(rows[0]?.oldText).toBe('否')
    expect(rows[0]?.newText).toBe('是')
  })
})

describe('DiffView', () => {
  it('字段名走中文标签字典', () => {
    render(<DiffView oldValue={{ name: 'A' }} newValue={{ name: 'B' }} />)
    expect(screen.getByText('名称')).toBeInTheDocument()
  })

  it('old/new 均为空时提示无内容', () => {
    render(<DiffView oldValue={null} newValue={null} />)
    expect(screen.getByText('无字段变更内容')).toBeInTheDocument()
  })
})
