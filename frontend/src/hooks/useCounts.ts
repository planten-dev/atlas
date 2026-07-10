import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { submitted } from '@/hooks/mutation-result'

export type OperationCountResponse = components['schemas']['OperationCountResponse']

export function countsListOptions(search: {
  status_filter?: 'active' | 'voided'
  sales_record_line_id?: string
  sales_record_id?: string
  page_number?: number
  page_size?: number
}) {
  return queryOptions({
    queryKey: ['operation-counts', 'list', search],
    queryFn: async () => {
      const data = unwrap(
        await client.GET('/api/v1/sales-record-operation-counts/list', {
          params: { query: search },
        }),
      )
      return { items: data.operation_counts, totalCount: data.total_count }
    },
  })
}

/** 次数账户以明细行 id 为键;无账户的行返回 404。 */
export function countDetailOptions(salesRecordLineId: string) {
  return queryOptions({
    queryKey: ['operation-counts', 'detail', salesRecordLineId],
    queryFn: async () =>
      unwrap(
        await client.GET(
          '/api/v1/sales-record-operation-counts/detail/{sales_record_line_id}',
          { params: { path: { sales_record_line_id: salesRecordLineId } } },
        ),
      ),
    retry: false,
  })
}

export function useUpdateOperationCount() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      salesRecordLineId,
      totalCount,
    }: {
      salesRecordLineId: string
      totalCount: number
    }) =>
      submitted(
        unwrap(
          await client.POST(
            '/api/v1/sales-record-operation-counts/update/{sales_record_line_id}',
            {
              params: { path: { sales_record_line_id: salesRecordLineId } },
              body: { total_count: totalCount },
            },
          ),
        ),
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['operation-counts'] })
      void queryClient.invalidateQueries({ queryKey: ['sales-records'] })
    },
  })
}
