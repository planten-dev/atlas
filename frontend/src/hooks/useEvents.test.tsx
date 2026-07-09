import { http, HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClientProvider, useQuery } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { server } from '@/mocks/server'
import { eventsListOptions, useReviewEvent } from '@/hooks/useEvents'
import { ApiError } from '@/api/error'
import { createTestQueryClient } from '@/test/utils'

const makeEvent = (overrides: Record<string, unknown> = {}) => ({
  id: 'e1',
  resource_type: 'products',
  resource_id: null,
  actor_user_id: 'u1',
  event_type: 0,
  approval_status: 1,
  required_approval_count: 2,
  required_approver_ids: [],
  custom_type: null,
  target_event_id: null,
  old_value: null,
  new_value: { name: 'A' },
  remark: null,
  created_at: '2026-07-08T00:00:00Z',
  updated_at: null,
  ...overrides,
})

function wrapperWith(queryClient = createTestQueryClient()) {
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  )
  return { Wrapper, queryClient }
}

describe('eventsListOptions', () => {
  it('归一 events 数组键为 items/totalCount', async () => {
    server.use(
      http.get('/api/v1/events/list', () =>
        HttpResponse.json({
          events: [makeEvent()],
          page_number: 1,
          page_size: 20,
          total_count: 41,
        }),
      ),
    )
    const { Wrapper } = wrapperWith()
    const { result } = renderHook(() => useQuery(eventsListOptions({ approval_status: 1 })), {
      wrapper: Wrapper,
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.items).toHaveLength(1)
    expect(result.current.data?.totalCount).toBe(41)
  })
})

describe('useReviewEvent', () => {
  it('approve 成功后失效 events 缓存', async () => {
    server.use(
      http.post('/api/v1/events/approve/e1', () =>
        HttpResponse.json(makeEvent({ approval_status: 2 })),
      ),
    )
    const { Wrapper, queryClient } = wrapperWith()
    queryClient.setQueryData(['events', 'badge'], 5)

    const { result } = renderHook(() => useReviewEvent('approve'), { wrapper: Wrapper })
    result.current.mutate({ eventId: 'e1', remark: 'ok' })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    expect(
      queryClient.getQueryState(['events', 'badge'])?.isInvalidated,
    ).toBe(true)
  })

  it('event_not_pending 错误路径抛 ApiError', async () => {
    server.use(
      http.post('/api/v1/events/reject/e1', () =>
        HttpResponse.json(
          { error: 'event_not_pending', message: 'already finalized' },
          { status: 409 },
        ),
      ),
    )
    const { Wrapper } = wrapperWith()
    const { result } = renderHook(() => useReviewEvent('reject'), { wrapper: Wrapper })
    result.current.mutate({ eventId: 'e1' })
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(result.current.error).toBeInstanceOf(ApiError)
    expect((result.current.error as ApiError).code).toBe('event_not_pending')
  })
})
