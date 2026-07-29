import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { submitted, type MutationOutcome } from '@/hooks/mutation-result'

export type SalesRecordResponse = components['schemas']['SalesRecordResponse']
export type SalesRecordLineResponse = components['schemas']['SalesRecordLineResponse']
export type SalesRecordLineInput = {
  product_id: string
  item_name: string
  operation_total_count?: number | null
  remark?: string | null
}
export type SalesRecordAllocationInput = { guide_user_id: string; allocation_ratio: string }
type StaffFields = {
  customer_id: string
  record_date: string
  handler_user_id: string
  expert_user_id?: string | null
  consultant_user_id?: string | null
  doctor_user_id?: string | null
  remark?: string | null
}
export type CreateDealRecordRequest = StaffFields & {
  total_amount: string
  received_amount: string
  customer_type: 'new' | 'returning'
  deal_type: 'non_salon' | 'salon'
  lines: SalesRecordLineInput[]
  allocations: SalesRecordAllocationInput[]
}
export type CreatePreServiceRecordRequest = StaffFields & {
  total_amount: string
  customer_type?: 'new' | 'returning' | null
  deal_type?: 'non_salon' | 'salon' | null
  lines: SalesRecordLineInput[]
}
export type CreateDebtCollectionRecordRequest = StaffFields & {
  received_amount: string
  allocations: SalesRecordAllocationInput[]
}

export interface SalesListSearch {
  status_filter?: 'active' | 'voided'
  record_type?: 'deal' | 'pre_service' | 'debt_collection'
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
      const data = unwrap(await client.GET('/api/v1/sales-records/list', { params: { query: search } }))
      return { items: data.sales_records, totalCount: data.total_count }
    },
  })
}
export function salesDetailOptions(id: string) {
  return queryOptions({ queryKey: ['sales-records', 'detail', id], queryFn: async () => unwrap(await client.GET('/api/v1/sales-records/detail/{sales_record_id}', { params: { path: { sales_record_id: id } } })) })
}

function useCreateSalesRecordMutation<T>(mutationFn: (body: T) => Promise<MutationOutcome<SalesRecordResponse>>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['sales-records'] })
      void queryClient.invalidateQueries({ queryKey: ['customers'] })
      void queryClient.invalidateQueries({ queryKey: ['operation-counts'] })
      void queryClient.invalidateQueries({ queryKey: ['sales-performance'] })
    },
  })
}
export function useCreateDeal() {
  return useCreateSalesRecordMutation<CreateDealRecordRequest>(async (body) =>
    submitted(unwrap(await client.POST('/api/v1/sales-records/create-deal', { body }))),
  )
}
export function useCreatePreService() {
  return useCreateSalesRecordMutation<CreatePreServiceRecordRequest>(async (body) =>
    submitted(unwrap(await client.POST('/api/v1/sales-records/create-pre-service', { body }))),
  )
}
export function useCreateDebtCollection() {
  return useCreateSalesRecordMutation<CreateDebtCollectionRecordRequest>(async (body) =>
    submitted(unwrap(await client.POST('/api/v1/sales-records/create-debt-collection', { body }))),
  )
}
export function useVoidSalesRecord() {
  const queryClient = useQueryClient()
  return useMutation({ mutationFn: async (id: string) => submitted(unwrap(await client.POST('/api/v1/sales-records/void/{sales_record_id}', { params: { path: { sales_record_id: id } } }))), onSuccess: () => { void queryClient.invalidateQueries({ queryKey: ['sales-records'] }); void queryClient.invalidateQueries({ queryKey: ['customers'] }); void queryClient.invalidateQueries({ queryKey: ['operation-counts'] }); void queryClient.invalidateQueries({ queryKey: ['operation-usages'] }); void queryClient.invalidateQueries({ queryKey: ['sales-performance'] }) } })
}
