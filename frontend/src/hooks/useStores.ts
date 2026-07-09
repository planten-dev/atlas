import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { applied, type MutationOutcome } from '@/hooks/mutation-result'

export type StoreResponse = components['schemas']['StoreResponse']

export function storesListOptions(search: {
  status_filter?: 'active' | 'disabled'
  system_id?: string
  page_number?: number
  page_size?: number
}) {
  return queryOptions({
    queryKey: ['stores', 'list', search],
    queryFn: async () => {
      const data = unwrap(await client.GET('/api/v1/stores/list', { params: { query: search } }))
      return { items: data.stores, totalCount: data.total_count }
    },
  })
}

/** 选择器用:某体系下全部启用门店。 */
export function storeOptionsForPicker(systemId: string | undefined) {
  return queryOptions({
    queryKey: ['stores', 'picker', systemId ?? ''],
    queryFn: async () => {
      const data = unwrap(
        await client.GET('/api/v1/stores/list', {
          params: { query: { status_filter: 'active', system_id: systemId, page_size: 200 } },
        }),
      )
      return data.stores
    },
    enabled: Boolean(systemId),
    staleTime: 5 * 60_000,
  })
}

export function useCreateStore() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: components['schemas']['CreateStoreRequest']) =>
      applied(unwrap(await client.POST('/api/v1/stores/create', { body }))),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['stores'] }),
  })
}

export function useUpdateStore() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      storeId,
      body,
    }: {
      storeId: string
      body: components['schemas']['UpdateStoreRequest']
    }): Promise<MutationOutcome<StoreResponse>> =>
      applied(
        unwrap(
          await client.POST('/api/v1/stores/update/{store_id}', {
            params: { path: { store_id: storeId } },
            body,
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['stores'] }),
  })
}

export function useDisableStore() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (storeId: string) =>
      applied(
        unwrap(
          await client.POST('/api/v1/stores/disable/{store_id}', {
            params: { path: { store_id: storeId } },
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['stores'] }),
  })
}

export function useDeleteStore() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (storeId: string) => {
      await client.POST('/api/v1/stores/delete/{store_id}', {
        params: { path: { store_id: storeId } },
      })
      return applied(undefined)
    },
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['stores'] }),
  })
}
