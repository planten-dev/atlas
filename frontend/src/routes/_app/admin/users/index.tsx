import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import type { ColumnDef } from '@tanstack/react-table'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { DataTable } from '@/components/data-table/data-table'
import { DataTableToolbar } from '@/components/data-table/toolbar'
import { UserName } from '@/components/UserName'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import { meQueryOptions } from '@/auth/session'
import { usersListOptions, useUpdateUserStatus, type UserResponse } from '@/hooks/useUsers'
import { formatDateTime } from '@/lib/date'
import { notify } from '@/lib/notify'

const searchSchema = z.object({
  status_filter: z.enum(['active', 'disabled']).optional(),
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

export const Route = createFileRoute('/_app/admin/users/')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'users:read'),
  staticData: { desktopOnly: true },
  component: UsersPage,
})

function UsersPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(usersListOptions(search))
  const { data: me } = useQuery(meQueryOptions)
  const statusMutation = useUpdateUserStatus()
  const rows = query.data?.items ?? []
  const [disabling, setDisabling] = useState<UserResponse | null>(null)

  const columns: ColumnDef<UserResponse>[] = [
    {
      accessorKey: 'id',
      header: '姓名',
      cell: ({ row }) => <UserName userId={row.original.id} />,
    },
    { accessorKey: 'dingtalk_user_id', header: '钉钉ID' },
    {
      accessorKey: 'last_login_at',
      header: '上次登录',
      cell: ({ row }) => formatDateTime(row.original.last_login_at),
    },
    {
      accessorKey: 'status',
      header: '状态',
      cell: ({ row }) =>
        row.original.status === 'disabled' ? (
          <Badge variant="destructive">停用</Badge>
        ) : (
          <Badge variant="outline">启用</Badge>
        ),
    },
    {
      id: 'actions',
      header: '操作',
      meta: { align: 'right' },
      cell: ({ row }) => {
        // 后端同样拒绝自停用(cannot_disable_self),这里直接不给入口
        if (row.original.id === me?.id) {
          return <span className="text-xs text-muted-foreground">当前账号</span>
        }
        return (
          <Guard perm="users:write">
            {row.original.status === 'active' ? (
              <Button
                variant="destructive"
                size="xs"
                onClick={(e) => {
                  e.stopPropagation()
                  setDisabling(row.original)
                }}
              >
                停用
              </Button>
            ) : (
              <Button
                variant="outline"
                size="xs"
                onClick={(e) => {
                  e.stopPropagation()
                  statusMutation.mutate(
                    { userId: row.original.id, status: 'active' },
                    {
                      onSuccess: () => notify.success('用户已启用'),
                      onError: (error) => notify.error(error),
                    },
                  )
                }}
              >
                启用
              </Button>
            )}
          </Guard>
        )
      },
    },
  ]

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">用户</h1>
      <AlertDialog open={disabling !== null} onOpenChange={(open) => !open && setDisabling(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              停用用户 “{disabling && <UserName userId={disabling.id} />}”?
            </AlertDialogTitle>
            <AlertDialogDescription>
              停用后该用户将立即退出登录且无法再登录系统,可随时重新启用。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (!disabling) return
                statusMutation.mutate(
                  { userId: disabling.id, status: 'disabled' },
                  {
                    onSuccess: () => notify.success('用户已停用'),
                    onError: (error) => notify.error(error),
                  },
                )
                setDisabling(null)
              }}
            >
              确认停用
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <DataTableToolbar>
        <Select
          items={[
            { value: 'all', label: '全部状态' },
            { value: 'active', label: '启用' },
            { value: 'disabled', label: '停用' },
          ]}
          value={search.status_filter ?? 'all'}
          onValueChange={(value) => {
            void navigate({
              search: (prev) => ({
                ...prev,
                status_filter: value === 'all' ? undefined : (value as 'active' | 'disabled'),
                page_number: 1,
              }),
            })
          }}
        >
          <SelectTrigger className="w-40">
            <SelectValue placeholder="状态" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">全部状态</SelectItem>
            <SelectItem value="active">启用</SelectItem>
            <SelectItem value="disabled">停用</SelectItem>
          </SelectContent>
        </Select>
      </DataTableToolbar>
      <DataTable
        tableId="admin-users"
        columns={columns}
        data={rows}
        totalCount={query.data?.totalCount ?? 0}
        isLoading={query.isLoading}
        page={{ pageNumber: search.page_number, pageSize: search.page_size }}
        onPageChange={(page) => {
          void navigate({
            search: (prev) => ({ ...prev, page_number: page.pageNumber, page_size: page.pageSize }),
          })
        }}
        onRowClick={(row) => {
          void navigate({ to: '/admin/users/$userId', params: { userId: row.id } })
        }}
      />
    </div>
  )
}
