import { http, HttpResponse } from 'msw'
import { describe, expect, it, vi } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClientProvider, useQuery } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { server } from '@/mocks/server'
import { paymentsListOptions, useCollectPayment, useVoidPayment } from '@/hooks/usePayments'
import { ApiError } from '@/api/error'
import { createTestQueryClient } from '@/test/utils'

function makeWrapper() {
  const queryClient = createTestQueryClient()
  return {
    queryClient,
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    ),
  }
}

function paymentPayload(id: string) {
  return {
    id,
    sales_record_id: 'r1',
    payment_type: 'collection',
    paid_amount: '100.00',
    paid_at: '2026-07-09T10:00:00Z',
    performance_status: 'pending',
    status: 'active',
    remark: null,
    created_by_user_id: 'u1',
    allocations: [],
    created_at: '2026-07-09T10:00:00Z',
    updated_at: '2026-07-09T10:00:00Z',
  }
}

function eventPayload(id: string) {
  return { id, resource_type: 'sales:payments', event_type: 0, approval_status: 1 }
}

describe('paymentsListOptions', () => {
  it('归一 sales_payments → items/totalCount', async () => {
    server.use(
      http.get('/api/v1/sales-payments/list', () =>
        HttpResponse.json({
          sales_payments: [paymentPayload('p1')],
          page_number: 1,
          page_size: 20,
          total_count: 1,
        }),
      ),
    )
    const { wrapper } = makeWrapper()
    const { result } = renderHook(() => useQuery(paymentsListOptions({})), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.items.map((p) => p.id)).toEqual(['p1'])
    expect(result.current.data?.totalCount).toBe(1)
  })
})

describe('useCollectPayment', () => {
  it('POST collect 并双失效 sales-payments + sales-records', async () => {
    let requestBody: unknown = null
    server.use(
      http.post('/api/v1/sales-payments/collect', async ({ request }) => {
        requestBody = await request.json()
        return HttpResponse.json(eventPayload('e2'), { status: 202 })
      }),
    )
    const { queryClient, wrapper } = makeWrapper()
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries')
    const { result } = renderHook(() => useCollectPayment(), { wrapper })
    result.current.mutate({
      sales_record_id: 'r1',
      paid_amount: '100.00',
      paid_at: '2026-07-09T10:00:00Z',
      allocations: [{ guide_user_id: 'g1', allocation_ratio: '100' }],
      remark: null,
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(requestBody).toEqual({
      sales_record_id: 'r1',
      paid_amount: '100.00',
      paid_at: '2026-07-09T10:00:00Z',
      allocations: [{ guide_user_id: 'g1', allocation_ratio: '100' }],
      remark: null,
    })
    const keys = invalidateSpy.mock.calls.map((call) => JSON.stringify(call[0]?.queryKey))
    expect(keys).toContain(JSON.stringify(['sales-payments']))
    expect(keys).toContain(JSON.stringify(['sales-records']))
  })

  it('409 payment_exceeds_outstanding 错误路径', async () => {
    server.use(
      http.post('/api/v1/sales-payments/collect', () =>
        HttpResponse.json(
          { error: 'payment_exceeds_outstanding', message: 'too much' },
          { status: 409 },
        ),
      ),
    )
    const { wrapper } = makeWrapper()
    const { result } = renderHook(() => useCollectPayment(), { wrapper })
    result.current.mutate({
      sales_record_id: 'r1',
      paid_amount: '999.00',
      paid_at: '2026-07-09T10:00:00Z',
      allocations: [{ guide_user_id: 'g1', allocation_ratio: '100' }],
      remark: null,
    })
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect((result.current.error as ApiError).code).toBe('payment_exceeds_outstanding')
  })
})

describe('useVoidPayment', () => {
  it('POST void 成功', async () => {
    server.use(
      http.post('/api/v1/sales-payments/void/p1', () =>
        HttpResponse.json(eventPayload('e1'), { status: 202 }),
      ),
    )
    const { wrapper } = makeWrapper()
    const { result } = renderHook(() => useVoidPayment(), { wrapper })
    result.current.mutate('p1')
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
  })
})
