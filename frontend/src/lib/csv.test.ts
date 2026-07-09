import { describe, expect, it } from 'vitest'
import { buildCsv, type CsvColumn } from '@/lib/csv'

interface Row {
  name: string
  amount: string
}

const columns: CsvColumn<Row>[] = [
  { header: '姓名', value: (r) => r.name },
  { header: '金额', value: (r) => r.amount },
]

describe('buildCsv', () => {
  it('以 UTF-8 BOM 开头、CRLF 分隔', () => {
    const csv = buildCsv(columns, [{ name: '张三', amount: '1.00' }])
    expect(csv.charCodeAt(0)).toBe(0xfeff)
    expect(csv).toContain('姓名,金额\r\n')
    expect(csv.endsWith('\r\n')).toBe(true)
  })

  it('转义逗号、引号与换行', () => {
    const csv = buildCsv(columns, [
      { name: 'a,b', amount: '1.00' },
      { name: 'say "hi"', amount: '2.00' },
      { name: 'line1\nline2', amount: '3.00' },
    ])
    expect(csv).toContain('"a,b",1.00')
    expect(csv).toContain('"say ""hi""",2.00')
    expect(csv).toContain('"line1\nline2",3.00')
  })

  it('null/undefined 输出为空串', () => {
    const cols: CsvColumn<{ v: string | null }>[] = [{ header: 'v', value: (r) => r.v }]
    const csv = buildCsv(cols, [{ v: null }])
    const lines = csv.split('\r\n')
    expect(lines[1]).toBe('')
  })
})
