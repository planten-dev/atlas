import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { applied, type MutationOutcome } from '@/hooks/mutation-result'

export type CustomerResponse = components['schemas']['CustomerResponse']

export interface CustomersListSearch {
  status_filter?: 'active' | 'disabled'
  system_id?: string
  store_id?: string
  creator_user_id?: string
  name_keyword?: string
  page_number?: number
  page_size?: number
}

export function customersListOptions(search: CustomersListSearch) {
  return queryOptions({
    queryKey: ['customers', 'list', search],
    queryFn: async () => {
      const data = unwrap(
        await client.GET('/api/v1/customers/list', { params: { query: search } }),
      )
      return { items: data.customers, totalCount: data.total_count }
    },
  })
}

export function customerDetailOptions(customerId: string) {
  return queryOptions({
    queryKey: ['customers', 'detail', customerId],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/customers/detail/{customer_id}', {
          params: { path: { customer_id: customerId } },
        }),
      ),
  })
}

export function useCreateCustomer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (
      body: components['schemas']['CreateCustomerRequest'],
    ): Promise<MutationOutcome<CustomerResponse>> =>
      applied(unwrap(await client.POST('/api/v1/customers/create', { body }))),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['customers'] }),
  })
}

export function useUpdateCustomer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      customerId,
      body,
    }: {
      customerId: string
      body: components['schemas']['UpdateCustomerRequest']
    }): Promise<MutationOutcome<CustomerResponse>> =>
      applied(
        unwrap(
          await client.POST('/api/v1/customers/update/{customer_id}', {
            params: { path: { customer_id: customerId } },
            body,
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['customers'] }),
  })
}

export function useDisableCustomer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (customerId: string) =>
      applied(
        unwrap(
          await client.POST('/api/v1/customers/disable/{customer_id}', {
            params: { path: { customer_id: customerId } },
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['customers'] }),
  })
}
