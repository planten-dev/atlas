import { http, HttpResponse } from 'msw'
import { describe, expect, it, vi } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClientProvider, useQuery } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { server } from '@/mocks/server'
import { pendingPerformanceOptions, usePostPerformanceBatch } from '@/hooks/usePerformance'
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

describe('performance hooks', () => {
  it('查询指定月份待入账收款', async () => {
    let month = ''
    server.use(
      http.get('/api/v1/sales-performance/pending', ({ request }) => {
        month = new URL(request.url).searchParams.get('period_month') ?? ''
        return HttpResponse.json({ sales_records: [], page_number: 1, page_size: 200, total_count: 0 })
      }),
    )
    const { wrapper } = makeWrapper()
    const { result } = renderHook(() => useQuery(pendingPerformanceOptions('2026-07-01')), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(month).toBe('2026-07-01')
  })

  it('批量入账后失效业绩和收款查询', async () => {
    let body: unknown
    server.use(
      http.post('/api/v1/sales-performance/batches', async ({ request }) => {
        body = await request.json()
        return HttpResponse.json({
          id: 'b1', period_month: '2026-07-01', batch_type: 'posting', record_count: 1,
          expert_amount: '100.00', guide_amount: '100.00', total_amount: '200.00',
          posted_by_user_id: 'u1', posted_at: '2026-07-10T00:00:00Z', created_at: '2026-07-10T00:00:00Z',
        }, { status: 201 })
      }),
    )
    const { queryClient, wrapper } = makeWrapper()
    const invalidateSpy = vi.spyOn(queryClient, 'invalidateQueries')
    const { result } = renderHook(() => usePostPerformanceBatch(), { wrapper })
    result.current.mutate({ period_month: '2026-07-01', sales_record_ids: ['p1'] })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(body).toEqual({ period_month: '2026-07-01', sales_record_ids: ['p1'] })
    const keys = invalidateSpy.mock.calls.map((call) => JSON.stringify(call[0]?.queryKey))
    expect(keys).toContain(JSON.stringify(['sales-performance']))
    expect(keys).toContain(JSON.stringify(['sales-records']))
  })
})
