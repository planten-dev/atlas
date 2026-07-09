import { createFileRoute, Link } from '@tanstack/react-router'
import { useSuspenseQuery } from '@tanstack/react-query'
import { ArrowLeft, RefreshCw } from 'lucide-react'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import { userProfileQueryOptions } from '@/hooks/useUserProfile'
import { useSyncUserProfile } from '@/hooks/useUsers'
import { notify } from '@/lib/notify'

export const Route = createFileRoute('/_app/admin/users/$userId')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'users:read'),
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(userProfileQueryOptions(params.userId)),
  staticData: { desktopOnly: true },
  component: UserDetailPage,
})

function UserDetailPage() {
  const { userId } = Route.useParams()
  const { data: profile } = useSuspenseQuery(userProfileQueryOptions(userId))
  const syncMutation = useSyncUserProfile()

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="icon-sm"
          render={<Link to="/admin/users" search={{}} aria-label="返回" />}
        >
          <ArrowLeft />
        </Button>
        <h1 className="flex-1 text-xl font-semibold">用户详情</h1>
        <Guard perm="users:write">
          <Button
            variant="outline"
            size="sm"
            disabled={syncMutation.isPending}
            onClick={() => {
              syncMutation.mutate(userId, {
                onSuccess: () => notify.success('已从钉钉同步资料'),
                onError: (error) => notify.error(error),
              })
            }}
          >
            <RefreshCw />
            从钉钉同步
          </Button>
        </Guard>
      </div>

      <Card>
        <CardContent className="flex items-center gap-4 py-4">
          <Avatar className="size-14">
            <AvatarImage src={profile.avatar_url ?? undefined} />
            <AvatarFallback>{profile.name?.slice(0, 1) ?? '?'}</AvatarFallback>
          </Avatar>
          <div className="flex flex-col gap-0.5 text-sm">
            <p className="text-base font-medium">{profile.name ?? '未同步'}</p>
            <p className="text-muted-foreground">
              {[profile.title, profile.job_number, profile.mobile].filter(Boolean).join(' · ') || '-'}
            </p>
            <p className="text-muted-foreground">{profile.email ?? profile.org_email ?? ''}</p>
          </div>
          <div className="ml-auto">
            {profile.is_active === false ? (
              <Badge variant="destructive">离职/停用</Badge>
            ) : (
              <Badge variant="outline">在职</Badge>
            )}
          </div>
        </CardContent>
      </Card>

    </div>
  )
}
