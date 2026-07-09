export interface CsvColumn<T> {
  header: string
  value: (row: T) => string | number | null | undefined
}

function escapeCell(raw: string): string {
  if (/[",\r\n]/.test(raw)) {
    return `"${raw.replaceAll('"', '""')}"`
  }
  return raw
}

/** 生成 CSV 文本:UTF-8 BOM(Excel 中文兼容)+ CRLF。 */
export function buildCsv<T>(columns: readonly CsvColumn<T>[], rows: readonly T[]): string {
  const lines = [
    columns.map((c) => escapeCell(c.header)).join(','),
    ...rows.map((row) =>
      columns.map((c) => escapeCell(String(c.value(row) ?? ''))).join(','),
    ),
  ]
  return '﻿' + lines.join('\r\n') + '\r\n'
}

export function downloadCsv(filename: string, content: string): void {
  const blob = new Blob([content], { type: 'text/csv;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename.endsWith('.csv') ? filename : `${filename}.csv`
  document.body.appendChild(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
}
