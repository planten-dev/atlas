import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { applied, type MutationOutcome } from '@/hooks/mutation-result'

export type SalesRecordResponse = components['schemas']['SalesRecordResponse']
export type CreateSalesRecordRequest = components['schemas']['CreateSalesRecordRequest']
export type UpdateSalesRecordRequest = components['schemas']['UpdateSalesRecordRequest']

export interface SalesListSearch {
  status_filter?: 'active' | 'voided'
  record_group_id?: string
  customer_id?: string
  department_id?: string
  system_id?: string
  store_id?: string
  handler_user_id?: string
  content_category_id?: string
  sale_date_from?: string
  sale_date_to?: string
  page_number?: number
  page_size?: number
}

export function salesListOptions(search: SalesListSearch) {
  return queryOptions({
    queryKey: ['sales-records', 'list', search],
    queryFn: async () => {
      const data = unwrap(
        await client.GET('/api/v1/sales-records/list', { params: { query: search } }),
      )
      return { items: data.sales_records, totalCount: data.total_count }
    },
  })
}

export function salesDetailOptions(salesRecordId: string) {
  return queryOptions({
    queryKey: ['sales-records', 'detail', salesRecordId],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/sales-records/detail/{sales_record_id}', {
          params: { path: { sales_record_id: salesRecordId } },
        }),
      ),
  })
}

export function useCreateSalesBatch() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (records: CreateSalesRecordRequest[]) =>
      applied(unwrap(await client.POST('/api/v1/sales-records/create-batch', { body: { records } }))),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['sales-records'] }),
  })
}

export function useUpdateSalesRecord() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      salesRecordId,
      body,
    }: {
      salesRecordId: string
      body: UpdateSalesRecordRequest
    }): Promise<MutationOutcome<SalesRecordResponse>> =>
      applied(
        unwrap(
          await client.POST('/api/v1/sales-records/update/{sales_record_id}', {
            params: { path: { sales_record_id: salesRecordId } },
            body,
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['sales-records'] }),
  })
}

export function useVoidSalesRecord() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (salesRecordId: string) =>
      applied(
        unwrap(
          await client.POST('/api/v1/sales-records/void/{sales_record_id}', {
            params: { path: { sales_record_id: salesRecordId } },
          }),
        ),
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['sales-records'] })
      void queryClient.invalidateQueries({ queryKey: ['operation-counts'] })
    },
  })
}
