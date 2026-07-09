import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'

export type EventResponse = components['schemas']['EventResponse']
export type EventType = EventResponse['event_type']
export type ApprovalStatus = EventResponse['approval_status']

export interface EventsListSearch {
  resource_type?: string
  resource_id?: string
  event_type?: EventType
  approval_status?: ApprovalStatus
  custom_type?: string
  target_event_id?: string
  page_number?: number
  page_size?: number
}

export function eventsListOptions(search: EventsListSearch) {
  return queryOptions({
    queryKey: ['events', 'list', search],
    queryFn: async () => {
      const data = unwrap(
        await client.GET('/api/v1/events/list', { params: { query: search } }),
      )
      return {
        items: data.events,
        totalCount: data.total_count,
      }
    },
  })
}

export function eventDetailOptions(eventId: string) {
  return queryOptions({
    queryKey: ['events', 'detail', eventId],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/events/detail/{event_id}', {
          params: { path: { event_id: eventId } },
        }),
      ),
  })
}

/** 审核历史:target_event_id 反查 approve/reject 事件(设计 §9)。 */
export function reviewHistoryOptions(targetEventId: string) {
  return queryOptions({
    queryKey: ['events', 'list', { target_event_id: targetEventId }],
    queryFn: async () => {
      const data = unwrap(
        await client.GET('/api/v1/events/list', {
          params: { query: { target_event_id: targetEventId, page_size: 200 } },
        }),
      )
      return data.events
    },
  })
}

export function useReviewEvent(action: 'approve' | 'reject') {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ eventId, remark }: { eventId: string; remark?: string }) => {
      const path =
        action === 'approve'
          ? ('/api/v1/events/approve/{event_id}' as const)
          : ('/api/v1/events/reject/{event_id}' as const)
      return unwrap(
        await client.POST(path, {
          params: { path: { event_id: eventId } },
          body: { remark: remark || null },
        }),
      )
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['events'] })
    },
  })
}
