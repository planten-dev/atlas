import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { applied, type MutationOutcome } from '@/hooks/mutation-result'

export type SystemResponse = components['schemas']['SystemResponse']

export function systemsListOptions(search: {
  status_filter?: 'active' | 'disabled'
  page_number?: number
  page_size?: number
}) {
  return queryOptions({
    queryKey: ['systems', 'list', search],
    queryFn: async () => {
      const data = unwrap(
        await client.GET('/api/v1/systems/list', { params: { query: search } }),
      )
      return { items: data.systems, totalCount: data.total_count }
    },
  })
}

/** 选择器用:全部启用体系(数量小,单页 200 足够)。 */
export const systemOptionsForPicker = queryOptions({
  queryKey: ['systems', 'picker'],
  queryFn: async () => {
    const data = unwrap(
      await client.GET('/api/v1/systems/list', {
        params: { query: { status_filter: 'active', page_size: 200 } },
      }),
    )
    return data.systems
  },
  staleTime: 5 * 60_000,
})

export function useCreateSystem() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: components['schemas']['CreateSystemRequest']) =>
      applied(unwrap(await client.POST('/api/v1/systems/create', { body }))),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['systems'] }),
  })
}

export function useUpdateSystem() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      systemId,
      body,
    }: {
      systemId: string
      body: components['schemas']['UpdateSystemRequest']
    }): Promise<MutationOutcome<SystemResponse>> =>
      applied(
        unwrap(
          await client.POST('/api/v1/systems/update/{system_id}', {
            params: { path: { system_id: systemId } },
            body,
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['systems'] }),
  })
}

export function useDisableSystem() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (systemId: string) =>
      applied(
        unwrap(
          await client.POST('/api/v1/systems/disable/{system_id}', {
            params: { path: { system_id: systemId } },
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['systems'] }),
  })
}

export function useDeleteSystem() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (systemId: string) => {
      await client.POST('/api/v1/systems/delete/{system_id}', {
        params: { path: { system_id: systemId } },
      })
      return applied(undefined)
    },
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['systems'] }),
  })
}
