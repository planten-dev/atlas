import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { submitted, type MutationOutcome } from '@/hooks/mutation-result'

export type SalesRecordResponse = components['schemas']['SalesRecordResponse']
export type SalesRecordLineResponse = components['schemas']['SalesRecordLineResponse']
export type CreateSaleRecordRequest = components['schemas']['CreateSaleRecordRequest']
export type CreateServiceRecordRequest = components['schemas']['CreateServiceRecordRequest']

export interface SalesListSearch {
  status_filter?: 'active' | 'voided'
  record_type?: 'sale' | 'service'
  customer_id?: string
  system_id?: string
  store_id?: string
  handler_user_id?: string
  record_date_from?: string
  record_date_to?: string
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

/** 销售记录:必带首次付款与分成;明细行按类别可能创建次数账户。 */
export function useCreateSale() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (
      body: CreateSaleRecordRequest,
    ): Promise<MutationOutcome<SalesRecordResponse>> =>
      submitted(unwrap(await client.POST('/api/v1/sales-records/create-sale', { body }))),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['sales-records'] })
      void queryClient.invalidateQueries({ queryKey: ['operation-counts'] })
      void queryClient.invalidateQueries({ queryKey: ['sales-payments'] })
    },
  })
}

/** 服务记录:无付款,行金额恒为 0。 */
export function useCreateService() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (
      body: CreateServiceRecordRequest,
    ): Promise<MutationOutcome<SalesRecordResponse>> =>
      submitted(unwrap(await client.POST('/api/v1/sales-records/create-service', { body }))),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['sales-records'] }),
  })
}

/** 作废级联明细行/次数账户/付款;存在有效耗用时后端 409 拒绝。 */
export function useVoidSalesRecord() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (salesRecordId: string) =>
      submitted(
        unwrap(
          await client.POST('/api/v1/sales-records/void/{sales_record_id}', {
            params: { path: { sales_record_id: salesRecordId } },
          }),
        ),
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['sales-records'] })
      void queryClient.invalidateQueries({ queryKey: ['operation-counts'] })
      void queryClient.invalidateQueries({ queryKey: ['operation-usages'] })
      void queryClient.invalidateQueries({ queryKey: ['sales-payments'] })
    },
  })
}
