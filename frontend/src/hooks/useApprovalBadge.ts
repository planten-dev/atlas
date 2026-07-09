import { useQuery } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import { usePermission } from '@/auth/PermissionProvider'

/**
 * 审批 badge:取 events/list?approval_status=1 的 total_count。
 * 近似值 —— 后端暂无"待我审批"语义(设计 §7 / 缺口 #8)。
 */
export function useApprovalBadgeCount(): number {
  const canRead = usePermission('events:read')
  const { data } = useQuery({
    queryKey: ['events', 'badge'],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/events/list', {
          params: { query: { approval_status: 1, page_number: 1, page_size: 1 } },
        }),
      ).total_count,
    enabled: canRead,
    refetchInterval: 60_000,
  })
  return data ?? 0
}
