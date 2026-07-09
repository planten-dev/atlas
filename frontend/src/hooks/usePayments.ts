import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { applied, type MutationOutcome } from '@/hooks/mutation-result'

export type SalesPaymentResponse = components['schemas']['SalesPaymentResponse']
export type CreateCollectionPaymentRequest =
  components['schemas']['CreateCollectionPaymentRequest']

export interface PaymentsListSearch {
  status_filter?: 'active' | 'voided'
  payment_type?: 'initial' | 'collection'
  sales_record_id?: string
  paid_at_from?: string
  paid_at_to?: string
  page_number?: number
  page_size?: number
}

export function paymentsListOptions(search: PaymentsListSearch) {
  return queryOptions({
    queryKey: ['sales-payments', 'list', search],
    queryFn: async () => {
      const data = unwrap(
        await client.GET('/api/v1/sales-payments/list', { params: { query: search } }),
      )
      return { items: data.sales_payments, totalCount: data.total_count }
    },
  })
}

export function paymentDetailOptions(paymentId: string) {
  return queryOptions({
    queryKey: ['sales-payments', 'detail', paymentId],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/sales-payments/detail/{payment_id}', {
          params: { path: { payment_id: paymentId } },
        }),
      ),
  })
}

/** 付款变化会改变销售记录的已收/未收(计算字段),必须双失效。 */
function invalidatePaymentRelated(queryClient: ReturnType<typeof useQueryClient>) {
  void queryClient.invalidateQueries({ queryKey: ['sales-payments'] })
  void queryClient.invalidateQueries({ queryKey: ['sales-records'] })
}

/** 登记回款:金额不得超过未收金额(后端 409 payment_exceeds_outstanding)。 */
export function useCollectPayment() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (
      body: CreateCollectionPaymentRequest,
    ): Promise<MutationOutcome<SalesPaymentResponse>> =>
      applied(unwrap(await client.POST('/api/v1/sales-payments/collect', { body }))),
    onSuccess: () => invalidatePaymentRelated(queryClient),
  })
}

export function useVoidPayment() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (paymentId: string) =>
      applied(
        unwrap(
          await client.POST('/api/v1/sales-payments/void/{payment_id}', {
            params: { path: { payment_id: paymentId } },
          }),
        ),
      ),
    onSuccess: () => invalidatePaymentRelated(queryClient),
  })
}
