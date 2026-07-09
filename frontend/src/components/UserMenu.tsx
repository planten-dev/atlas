import { Link, useNavigate } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { LogOut, User } from 'lucide-react'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { logout, meQueryOptions } from '@/auth/session'
import { useUserProfile } from '@/hooks/useUserProfile'
import { notify } from '@/lib/notify'

/** 头部右上角头像菜单:个人中心 + 退出登录。 */
export function UserMenu() {
  const { data: me } = useQuery(meQueryOptions)
  const { data: profile } = useUserProfile(me?.id)
  const queryClient = useQueryClient()
  const navigate = useNavigate()

  const logoutMutation = useMutation({
    mutationFn: logout,
    onSuccess: () => {
      queryClient.clear()
      void navigate({ to: '/login' })
    },
    onError: (error) => notify.error(error),
  })

  const name = profile?.name ?? me?.dingtalk_user_id ?? ''

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <button
            type="button"
            aria-label="用户菜单"
            className="rounded-full outline-none transition-opacity hover:opacity-80 focus-visible:ring-2 focus-visible:ring-ring"
          />
        }
      >
        <Avatar className="size-8">
          <AvatarImage src={profile?.avatar_url ?? undefined} alt={name} />
          <AvatarFallback>{name.slice(0, 1) || '?'}</AvatarFallback>
        </Avatar>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-44">
        {/* Base UI 的 GroupLabel 必须在 Menu.Group 内 */}
        <DropdownMenuGroup>
          <DropdownMenuLabel>
            <p className="truncate font-medium">{name || '未登录'}</p>
            {profile?.title && (
              <p className="truncate text-xs font-normal text-muted-foreground">{profile.title}</p>
            )}
          </DropdownMenuLabel>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          render={
            <Link to="/me">
              <User />
              个人中心
            </Link>
          }
        />
        <DropdownMenuItem
          variant="destructive"
          disabled={logoutMutation.isPending}
          onClick={() => logoutMutation.mutate()}
        >
          <LogOut />
          退出登录
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
