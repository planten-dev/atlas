import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'

export type PendingPerformanceRecord = components['schemas']['PendingPerformanceRecordResponse']
export type PerformanceSummary = components['schemas']['PerformanceSummaryResponse']
export type PerformanceEntry = components['schemas']['PerformanceEntryResponse']
export type PerformanceBatch = components['schemas']['PerformanceBatchResponse']
export type CreatePerformanceBatchRequest = components['schemas']['CreatePerformanceBatchRequest']

export interface PerformanceFilters {
  performance_date_from: string
  performance_date_to: string
  user_id?: string
  performance_role?: 'expert' | 'guide'
  system_id?: string
  store_id?: string
  entry_type?: 'earning' | 'reversal'
}

export function pendingPerformanceOptions(periodMonth: string) {
  return queryOptions({
    queryKey: ['sales-performance', 'pending', periodMonth],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/sales-performance/pending', {
          params: { query: { period_month: periodMonth, page_size: 200 } },
        }),
      ),
  })
}

export function performanceSummaryOptions(
  filters: PerformanceFilters,
  pageNumber: number,
  pageSize: number,
) {
  return queryOptions({
    queryKey: ['sales-performance', 'summary', filters, pageNumber, pageSize],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/sales-performance/summary', {
          params: { query: { ...filters, page_number: pageNumber, page_size: pageSize } },
        }),
      ),
  })
}

export function performanceEntriesOptions(
  filters: PerformanceFilters,
  pageNumber: number,
  pageSize: number,
) {
  return queryOptions({
    queryKey: ['sales-performance', 'entries', filters, pageNumber, pageSize],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/sales-performance/entries', {
          params: { query: { ...filters, page_number: pageNumber, page_size: pageSize } },
        }),
      ),
  })
}

export function performanceBatchesOptions(periodMonth: string) {
  return queryOptions({
    queryKey: ['sales-performance', 'batches', periodMonth],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/sales-performance/batches', {
          params: { query: { period_month: periodMonth, page_size: 50 } },
        }),
      ),
  })
}

export async function exportPerformance(filters: PerformanceFilters): Promise<void> {
  const result = await client.GET('/api/v1/sales-performance/export', {
    params: { query: filters },
    parseAs: 'blob',
  })
  const blob = unwrap(result) as unknown as Blob
  const filename = `sales-performance-${filters.performance_date_from}-to-${filters.performance_date_to}.xlsx`
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = filename
  anchor.click()
  URL.revokeObjectURL(url)
}

export function usePostPerformanceBatch() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreatePerformanceBatchRequest) =>
      unwrap(await client.POST('/api/v1/sales-performance/batches', { body })),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['sales-performance'] })
      void queryClient.invalidateQueries({ queryKey: ['sales-records'] })
    },
  })
}
