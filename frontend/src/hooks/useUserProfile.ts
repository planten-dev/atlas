import { queryOptions, useQuery } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'

export function userProfileQueryOptions(userId: string) {
  return queryOptions({
    queryKey: ['users', 'profile', userId],
    queryFn: async () =>
      unwrap(
        await client.GET('/api/v1/users/{user_id}/profile', {
          params: { path: { user_id: userId } },
        }),
      ),
    staleTime: 30 * 60_000,
  })
}

export function useUserProfile(userId: string | null | undefined) {
  return useQuery({
    ...userProfileQueryOptions(userId ?? ''),
    enabled: Boolean(userId),
  })
}
