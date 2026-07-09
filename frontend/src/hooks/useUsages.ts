import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { applied, type MutationOutcome } from '@/hooks/mutation-result'

export type OperationUsageResponse = components['schemas']['OperationUsageResponse']

export interface UsagesListSearch {
  status_filter?: 'active' | 'voided'
  sales_record_line_id?: string
  sales_record_id?: string
  operator_user_id?: string
  doctor_user_id?: string
  operated_at_from?: string
  operated_at_to?: string
  page_number?: number
  page_size?: number
}

export function usagesListOptions(search: UsagesListSearch) {
  return queryOptions({
    queryKey: ['operation-usages', 'list', search],
    queryFn: async () => {
      const data = unwrap(
        await client.GET('/api/v1/sales-record-operation-usages/list', {
          params: { query: search },
        }),
      )
      return { items: data.operation_usages, totalCount: data.total_count }
    },
  })
}

function invalidateUsageRelated(queryClient: ReturnType<typeof useQueryClient>) {
  void queryClient.invalidateQueries({ queryKey: ['operation-usages'] })
  void queryClient.invalidateQueries({ queryKey: ['operation-counts'] })
  void queryClient.invalidateQueries({ queryKey: ['sales-records'] })
}

export function useCreateUsage() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (
      body: components['schemas']['CreateOperationUsageRequest'],
    ): Promise<MutationOutcome<OperationUsageResponse>> =>
      applied(unwrap(await client.POST('/api/v1/sales-record-operation-usages/create', { body }))),
    onSuccess: () => invalidateUsageRelated(queryClient),
  })
}

/**
 * PatchField 三态语义:键缺失 = 不变;null = 清空(仅 doctor_user_id/remark 合法);
 * operated_at/operator_user_id/operation_count 传 null 会被 422 拒绝 —— 调用方只组装变更键。
 */
export function useUpdateUsage() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      usageId,
      body,
    }: {
      usageId: string
      body: components['schemas']['UpdateOperationUsageRequest']
    }): Promise<MutationOutcome<OperationUsageResponse>> =>
      applied(
        unwrap(
          await client.POST('/api/v1/sales-record-operation-usages/update/{usage_id}', {
            params: { path: { usage_id: usageId } },
            body,
          }),
        ),
      ),
    onSuccess: () => invalidateUsageRelated(queryClient),
  })
}

/** 作废耗用:后端回退次数账户(页面需提示)。 */
export function useVoidUsage() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (usageId: string) =>
      applied(
        unwrap(
          await client.POST('/api/v1/sales-record-operation-usages/void/{usage_id}', {
            params: { path: { usage_id: usageId } },
          }),
        ),
      ),
    onSuccess: () => invalidateUsageRelated(queryClient),
  })
}
