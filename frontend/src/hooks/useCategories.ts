import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { applied, type MutationOutcome } from '@/hooks/mutation-result'

export type ProductCategoryResponse = components['schemas']['ProductCategoryResponse']

export function categoriesListOptions(search: {
  status_filter?: 'active' | 'disabled'
  requires_operation_count_filter?: boolean
  page_number?: number
  page_size?: number
}) {
  return queryOptions({
    queryKey: ['product-categories', 'list', search],
    queryFn: async () => {
      const data = unwrap(
        await client.GET('/api/v1/product-categories/list', { params: { query: search } }),
      )
      return { items: data.categories, totalCount: data.total_count }
    },
  })
}

/** 选择器/销售表单用:全部启用类别(含 requires_operation_count)。 */
export const activeCategoriesOptions = queryOptions({
  queryKey: ['product-categories', 'picker'],
  queryFn: async () => {
    const data = unwrap(
      await client.GET('/api/v1/product-categories/list', {
        params: { query: { status_filter: 'active', page_size: 200 } },
      }),
    )
    return data.categories
  },
  staleTime: 5 * 60_000,
})

export function useCreateCategory() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (body: components['schemas']['CreateProductCategoryRequest']) =>
      applied(unwrap(await client.POST('/api/v1/product-categories/create', { body }))),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['product-categories'] }),
  })
}

export function useUpdateCategory() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      categoryId,
      body,
    }: {
      categoryId: string
      body: components['schemas']['UpdateProductCategoryRequest']
    }): Promise<MutationOutcome<ProductCategoryResponse>> =>
      applied(
        unwrap(
          await client.POST('/api/v1/product-categories/update/{category_id}', {
            params: { path: { category_id: categoryId } },
            body,
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['product-categories'] }),
  })
}

export function useDisableCategory() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (categoryId: string) =>
      applied(
        unwrap(
          await client.POST('/api/v1/product-categories/disable/{category_id}', {
            params: { path: { category_id: categoryId } },
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['product-categories'] }),
  })
}

export function useDeleteCategory() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (categoryId: string) => {
      await client.POST('/api/v1/product-categories/delete/{category_id}', {
        params: { path: { category_id: categoryId } },
      })
      return applied(undefined)
    },
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['product-categories'] }),
  })
}
