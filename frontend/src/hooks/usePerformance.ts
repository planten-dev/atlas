import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'

export type PendingPerformancePayment = components['schemas']['PendingPerformancePaymentResponse']
export type PerformanceSummary = components['schemas']['PerformanceSummaryResponse']
export type PerformanceEntry = components['schemas']['PerformanceEntryResponse']
export type PerformanceBatch = components['schemas']['PerformanceBatchResponse']
export type CreatePerformanceBatchRequest = components['schemas']['CreatePerformanceBatchRequest']

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

export function performanceSummaryOptions(periodMonth: string) {
  return queryOptions({
    queryKey: ['sales-performance', 'summary', periodMonth],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/sales-performance/summary', {
          params: { query: { period_month: periodMonth, page_size: 200 } },
        }),
      ),
  })
}

export function performanceEntriesOptions(periodMonth: string, userId?: string) {
  return queryOptions({
    queryKey: ['sales-performance', 'entries', periodMonth, userId],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/sales-performance/entries', {
          params: { query: { period_month: periodMonth, user_id: userId, page_size: 200 } },
        }),
      ),
    enabled: Boolean(userId),
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

export function usePostPerformanceBatch() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreatePerformanceBatchRequest) =>
      unwrap(await client.POST('/api/v1/sales-performance/batches', { body })),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['sales-performance'] })
      void queryClient.invalidateQueries({ queryKey: ['sales-payments'] })
    },
  })
}
