import { Fragment, useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { z } from 'zod'
import { ChevronDown, ChevronRight } from 'lucide-react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { DataTablePagination } from '@/components/data-table/pagination'
import { DataTableToolbar } from '@/components/data-table/toolbar'
import { DiffView } from '@/components/diff/DiffView'
import { UserName } from '@/components/UserName'
import { requirePerm } from '@/auth/route-guard'
import {
  eventsListOptions,
  type ApprovalStatus,
  type EventResponse,
  type EventType,
} from '@/hooks/useEvents'
import {
  APPROVAL_STATUS_LABELS,
  EVENT_TYPE_LABELS,
  approvalStatusLabel,
  eventTypeLabel,
  resourceTypeLabel,
} from '@/lib/labels'
import { formatDateTime } from '@/lib/date'

const eventTypeSchema = z.union([
  z.literal(0),
  z.literal(1),
  z.literal(2),
  z.literal(3),
  z.literal(4),
  z.literal(5),
])
const approvalStatusSchema = z.union([z.literal(0), z.literal(1), z.literal(2), z.literal(3)])

const searchSchema = z.object({
  resource_type: z.string().optional(),
  event_type: eventTypeSchema.optional(),
  approval_status: approvalStatusSchema.optional(),
  custom_type: z.string().optional(),
  page_number: z.number().int().min(1).default(1),
  page_size: z.number().int().min(1).max(200).default(20),
})

export const Route = createFileRoute('/_app/admin/audit')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'events:read'),
  staticData: { desktopOnly: true },
  component: AuditPage,
})

function AuditPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const query = useQuery(eventsListOptions(search))
  const rows = query.data?.items ?? []
  const [expandedId, setExpandedId] = useState<string | null>(null)

  const patchSearch = (patch: Partial<z.infer<typeof searchSchema>>) => {
    void navigate({ search: (prev) => ({ ...prev, ...patch, page_number: 1 }) })
  }

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-xl font-semibold">审计日志</h1>

      {/* 留存策略提示(设计 §7):滚动窗口 */}
      <Alert>
        <AlertTitle>滚动窗口</AlertTitle>
        <AlertDescription>
          审计日志包含全部事件(含无需审批的纯审计记录与自定义事件);已终态事件 180 天后由后端自动清除。
        </AlertDescription>
      </Alert>

      <DataTableToolbar
        exportConfig={{
          filename: `审计日志-${new Date().toISOString().slice(0, 10)}`,
          columns: [
            { header: '时间', value: (r: EventResponse) => formatDateTime(r.created_at) },
            { header: '资源类型', value: (r) => resourceTypeLabel(r.resource_type) },
            { header: '事件类型', value: (r) => eventTypeLabel(r.event_type) },
            { header: '审批状态', value: (r) => approvalStatusLabel(r.approval_status) },
            { header: '自定义类型', value: (r) => r.custom_type },
            { header: '资源ID', value: (r) => r.resource_id },
            { header: '备注', value: (r) => r.remark },
          ],
          rows,
        }}
      >
        <div className="grid w-full gap-2 md:grid-cols-4">
          <Input
            placeholder="资源类型(如 products)"
            defaultValue={search.resource_type ?? ''}
            onChange={(e) => patchSearch({ resource_type: e.target.value || undefined })}
          />
          <Select
            items={[
              { value: 'all', label: '全部事件类型' },
              ...Object.entries(EVENT_TYPE_LABELS).map(([value, label]) => ({ value, label })),
            ]}
            value={search.event_type !== undefined ? String(search.event_type) : 'all'}
            onValueChange={(value) =>
              patchSearch({
                event_type: value === 'all' ? undefined : (Number(value) as EventType),
              })
            }
          >
            <SelectTrigger className="w-full">
              <SelectValue placeholder="事件类型" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部事件类型</SelectItem>
              {Object.entries(EVENT_TYPE_LABELS).map(([value, label]) => (
                <SelectItem key={value} value={value}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            items={[
              { value: 'all', label: '全部审批状态' },
              ...Object.entries(APPROVAL_STATUS_LABELS).map(([value, label]) => ({ value, label })),
            ]}
            value={search.approval_status !== undefined ? String(search.approval_status) : 'all'}
            onValueChange={(value) =>
              patchSearch({
                approval_status: value === 'all' ? undefined : (Number(value) as ApprovalStatus),
              })
            }
          >
            <SelectTrigger className="w-full">
              <SelectValue placeholder="审批状态" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部审批状态</SelectItem>
              {Object.entries(APPROVAL_STATUS_LABELS).map(([value, label]) => (
                <SelectItem key={value} value={value}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Input
            placeholder="自定义类型(如 login)"
            defaultValue={search.custom_type ?? ''}
            onChange={(e) => patchSearch({ custom_type: e.target.value || undefined })}
          />
        </div>
      </DataTableToolbar>

      <div className="overflow-x-auto rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-8" />
              <TableHead>时间</TableHead>
              <TableHead>资源</TableHead>
              <TableHead>事件</TableHead>
              <TableHead>审批状态</TableHead>
              <TableHead>操作者</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {query.isLoading ? (
              <TableRow>
                <TableCell colSpan={6} className="h-24 text-center text-muted-foreground">
                  加载中…
                </TableCell>
              </TableRow>
            ) : rows.length === 0 ? (
              <TableRow>
                <TableCell colSpan={6} className="h-24 text-center text-muted-foreground">
                  没有匹配的事件
                </TableCell>
              </TableRow>
            ) : (
              rows.map((event) => (
                <Fragment key={event.id}>
                  <TableRow
                    className="cursor-pointer"
                    onClick={() => setExpandedId(expandedId === event.id ? null : event.id)}
                  >
                    <TableCell>
                      <Button variant="ghost" size="icon-xs" aria-label="展开详情">
                        {expandedId === event.id ? <ChevronDown /> : <ChevronRight />}
                      </Button>
                    </TableCell>
                    <TableCell>{formatDateTime(event.created_at)}</TableCell>
                    <TableCell>{resourceTypeLabel(event.resource_type)}</TableCell>
                    <TableCell>
                      <Badge variant="outline">{eventTypeLabel(event.event_type)}</Badge>
                      {event.custom_type && (
                        <Badge variant="secondary" className="ml-1">
                          {event.custom_type}
                        </Badge>
                      )}
                    </TableCell>
                    <TableCell>{approvalStatusLabel(event.approval_status)}</TableCell>
                    <TableCell>
                      <UserName userId={event.actor_user_id} />
                    </TableCell>
                  </TableRow>
                  {expandedId === event.id && (
                    <TableRow>
                      <TableCell colSpan={6} className="bg-muted/30">
                        <div className="flex flex-col gap-2 p-2">
                          <DiffView oldValue={event.old_value} newValue={event.new_value} />
                          {event.remark && (
                            <p className="text-sm text-muted-foreground">备注:{event.remark}</p>
                          )}
                          <p className="font-mono text-xs text-muted-foreground">
                            事件 {event.id}
                            {event.resource_id && ` · 资源 ${event.resource_id}`}
                            {event.target_event_id && ` · 关联事件 ${event.target_event_id}`}
                          </p>
                        </div>
                      </TableCell>
                    </TableRow>
                  )}
                </Fragment>
              ))
            )}
          </TableBody>
        </Table>
      </div>

      <DataTablePagination
        page={{ pageNumber: search.page_number, pageSize: search.page_size }}
        totalCount={query.data?.totalCount ?? 0}
        onPageChange={(page) => {
          void navigate({
            search: (prev) => ({ ...prev, page_number: page.pageNumber, page_size: page.pageSize }),
          })
        }}
      />
    </div>
  )
}
