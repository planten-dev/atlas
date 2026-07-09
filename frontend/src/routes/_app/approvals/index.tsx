import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import type { ColumnDef } from '@tanstack/react-table'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Badge } from '@/components/ui/badge'
import { DataTable } from '@/components/data-table/data-table'
import { UserName } from '@/components/UserName'
import { requirePerm } from '@/auth/route-guard'
import { eventsListOptions, type EventResponse } from '@/hooks/useEvents'
import { eventTypeLabel, resourceTypeLabel } from '@/lib/labels'
import { formatDateTime } from '@/lib/date'

const searchSchema = z.object({
  status: z.union([z.literal(1), z.literal(2), z.literal(3)]).default(1),
  resource_type: z.string().optional(),
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

export const Route = createFileRoute('/_app/approvals/')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'events:read'),
  staticData: { tab: 'approvals' },
  component: ApprovalsPage,
})

const STATUS_TABS = [
  { value: 1, label: '待审批' },
  { value: 2, label: '已通过' },
  { value: 3, label: '已驳回' },
] as const

const columns: ColumnDef<EventResponse>[] = [
  {
    accessorKey: 'created_at',
    header: '时间',
    cell: ({ row }) => formatDateTime(row.original.created_at),
  },
  {
    accessorKey: 'resource_type',
    header: '资源',
    cell: ({ row }) => resourceTypeLabel(row.original.resource_type),
  },
  {
    accessorKey: 'event_type',
    header: '事件',
    cell: ({ row }) => <Badge variant="outline">{eventTypeLabel(row.original.event_type)}</Badge>,
  },
  {
    accessorKey: 'actor_user_id',
    header: '发起人',
    cell: ({ row }) => <UserName userId={row.original.actor_user_id} />,
  },
]

function ApprovalsPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })

  const query = useQuery(
    eventsListOptions({
      approval_status: search.status,
      resource_type: search.resource_type,
      page_number: search.page_number,
      page_size: search.page_size,
    }),
  )

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">审批中心</h1>

      <Tabs
        value={String(search.status)}
        onValueChange={(value) => {
          void navigate({
            search: (prev) => ({
              ...prev,
              status: Number(value) as 1 | 2 | 3,
              page_number: 1,
            }),
          })
        }}
      >
        <TabsList>
          {STATUS_TABS.map((tab) => (
            <TabsTrigger key={tab.value} value={String(tab.value)}>
              {tab.label}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>

      <DataTable
        tableId="approvals"
        columns={columns}
        data={query.data?.items ?? []}
        totalCount={query.data?.totalCount ?? 0}
        isLoading={query.isLoading}
        page={{ pageNumber: search.page_number, pageSize: search.page_size }}
        onPageChange={(page) => {
          void navigate({
            search: (prev) => ({
              ...prev,
              page_number: page.pageNumber,
              page_size: page.pageSize,
            }),
          })
        }}
        onRowClick={(row) => {
          void navigate({ to: '/approvals/$eventId', params: { eventId: row.id } })
        }}
        emptyText="没有相应状态的审批事件"
      />
    </div>
  )
}
