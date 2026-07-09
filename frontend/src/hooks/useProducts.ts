import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { applied, type MutationOutcome } from '@/hooks/mutation-result'

export type ProductResponse = components['schemas']['ProductResponse']

export function productsListOptions(search: {
  status_filter?: 'active' | 'disabled'
  category_id?: string
  page_number?: number
  page_size?: number
}) {
  return queryOptions({
    queryKey: ['products', 'list', search],
    queryFn: async () => {
      const data = unwrap(await client.GET('/api/v1/products/list', { params: { query: search } }))
      return { items: data.products, totalCount: data.total_count }
    },
  })
}

// products 是设计文档点名的未来审批域:页面必须消费 submitted 分支,
// 后端接入审批后只改这里(返回 { kind: 'submitted', eventId })。
export function useCreateProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (
      body: components['schemas']['CreateProductRequest'],
    ): Promise<MutationOutcome<ProductResponse>> =>
      applied(unwrap(await client.POST('/api/v1/products/create', { body }))),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['products'] }),
  })
}

export function useUpdateProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      productId,
      body,
    }: {
      productId: string
      body: components['schemas']['UpdateProductRequest']
    }): Promise<MutationOutcome<ProductResponse>> =>
      applied(
        unwrap(
          await client.POST('/api/v1/products/update/{product_id}', {
            params: { path: { product_id: productId } },
            body,
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['products'] }),
  })
}

export function useDisableProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (productId: string) =>
      applied(
        unwrap(
          await client.POST('/api/v1/products/disable/{product_id}', {
            params: { path: { product_id: productId } },
          }),
        ),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['products'] }),
  })
}

export function useDeleteProduct() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (productId: string) => {
      await client.POST('/api/v1/products/delete/{product_id}', {
        params: { path: { product_id: productId } },
      })
      return applied(undefined)
    },
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['products'] }),
  })
}
