import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import type { ColumnDef } from '@tanstack/react-table'
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
  const statusMutation = useUpdateUserStatus()
  const rows = query.data?.items ?? []

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
      header: '',
      cell: ({ row }) => (
        <Guard perm="users:write">
          <Button
            variant="ghost"
            size="xs"
            onClick={(e) => {
              e.stopPropagation()
              statusMutation.mutate(
                {
                  userId: row.original.id,
                  status: row.original.status === 'active' ? 'disabled' : 'active',
                },
                {
                  onSuccess: () => notify.success('用户状态已更新'),
                  onError: (error) => notify.error(error),
                },
              )
            }}
          >
            {row.original.status === 'active' ? '停用' : '启用'}
          </Button>
        </Guard>
      ),
    },
  ]

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">用户</h1>
      <DataTableToolbar>
        <Select
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
