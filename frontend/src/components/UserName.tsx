import { useUserProfile } from '@/hooks/useUserProfile'

/** user_id → 姓名(经 /users/{id}/profile,长缓存);空值显示 '-'。 */
export function UserName({ userId }: { userId: string | null | undefined }) {
  const { data, isLoading } = useUserProfile(userId)
  if (!userId) return <span>-</span>
  if (isLoading) return <span className="text-muted-foreground">…</span>
  return <span>{data?.name ?? userId.slice(0, 8)}</span>
}
