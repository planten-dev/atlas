import {
  queryOptions,
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { fetchAllPages } from '@/lib/pagination'
import { userProfileQueryOptions } from '@/hooks/useUserProfile'

export type UserResponse = components['schemas']['UserResponse']
export type UserProfileResponse = components['schemas']['UserProfileResponse']

export function usersListOptions(search: {
  status_filter?: 'active' | 'disabled'
  page_number?: number
  page_size?: number
}) {
  return queryOptions({
    queryKey: ['users', 'list', search],
    queryFn: async () => {
      const data = unwrap(await client.GET('/api/v1/users/list', { params: { query: search } }))
      return { items: data.users, totalCount: data.total_count }
    },
  })
}

/** 人员选择器:后端缺 name_keyword(缺口 #3),全量拉取 + 前端过滤(内部规模可接受)。 */
export const allActiveUsersOptions = queryOptions({
  queryKey: ['users', 'picker'],
  queryFn: async () =>
    fetchAllPages(async (pageNumber, pageSize) => {
      const data = unwrap(
        await client.GET('/api/v1/users/list', {
          params: {
            query: { status_filter: 'active', page_number: pageNumber, page_size: pageSize },
          },
        }),
      )
      return { items: data.users, totalCount: data.total_count }
    }),
  staleTime: 10 * 60_000,
})

export function useUpdateUserStatus() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ userId, status }: { userId: string; status: 'active' | 'disabled' }) =>
      unwrap(
        await client.POST('/api/v1/users/update-status/{user_id}', {
          params: { path: { user_id: userId } },
          body: { target_status: status },
        }),
      ),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['users'] }),
  })
}

/** 从钉钉同步用户资料。 */
export function useSyncUserProfile() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (userId: string) =>
      unwrap(
        await client.POST('/api/v1/users/{user_id}/profile/sync', {
          params: { path: { user_id: userId } },
        }),
      ),
    onSuccess: (_, userId) =>
      void queryClient.invalidateQueries({ queryKey: ['users', 'profile', userId] }),
  })
}

export interface UserOption {
  id: string
  name: string
  title: string | null
}

/** 选择器数据:用户列表 + 各自 profile 的姓名(profile 查询有长缓存,去重后开销可控)。 */
export function useUserOptions(): { options: UserOption[]; isLoading: boolean } {
  const usersQuery = useQuery(allActiveUsersOptions)
  const users = usersQuery.data ?? []
  const profileQueries = useQueries({
    queries: users.map((u) => userProfileQueryOptions(u.id)),
  })
  const options: UserOption[] = users.map((u, i) => {
    const profile = profileQueries[i]?.data
    return {
      id: u.id,
      name: profile?.name ?? u.dingtalk_user_id,
      title: profile?.title ?? null,
    }
  })
  return {
    options,
    isLoading: usersQuery.isLoading,
  }
}
