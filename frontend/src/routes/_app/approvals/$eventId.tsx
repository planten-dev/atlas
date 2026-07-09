import { useState } from 'react'
import { createFileRoute, Link } from '@tanstack/react-router'
import { useQuery, useSuspenseQuery } from '@tanstack/react-query'
import { ArrowLeft } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Textarea } from '@/components/ui/textarea'
import { Separator } from '@/components/ui/separator'
import { DiffView } from '@/components/diff/DiffView'
import { UserName } from '@/components/UserName'
import { requirePerm } from '@/auth/route-guard'
import { usePermission } from '@/auth/PermissionProvider'
import {
  eventDetailOptions,
  reviewHistoryOptions,
  useReviewEvent,
} from '@/hooks/useEvents'
import {
  approvalStatusLabel,
  eventTypeLabel,
  resourceTypeLabel,
} from '@/lib/labels'
import { formatDateTime } from '@/lib/date'
import { notify } from '@/lib/notify'

export const Route = createFileRoute('/_app/approvals/$eventId')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'events:read'),
  loader: ({ context, params }) =>
    context.queryClient.ensureQueryData(eventDetailOptions(params.eventId)),
  component: ApprovalDetailPage,
})

const APPROVAL_STATUS_BADGE: Record<number, 'default' | 'secondary' | 'destructive' | 'outline'> = {
  0: 'outline',
  1: 'secondary',
  2: 'default',
  3: 'destructive',
}

function ApprovalDetailPage() {
  const { eventId } = Route.useParams()
  const { data: event } = useSuspenseQuery(eventDetailOptions(eventId))
  const historyQuery = useQuery(reviewHistoryOptions(eventId))

  // 动态权限:{resource_type}:approve 在权限集中才显示审批按钮(设计 §6)
  const canApprove = usePermission(`${event.resource_type}:approve`)
  const isPending = event.approval_status === 1

  const approveCount =
    historyQuery.data?.filter((e) => e.event_type === 3).length ?? 0
  const remainingVotes =
    event.required_approval_count !== null && event.required_approval_count !== undefined
      ? Math.max(0, event.required_approval_count - approveCount)
      : null

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon-sm" render={<Link to="/approvals" aria-label="返回" />}>
          <ArrowLeft />
        </Button>
        <h1 className="text-xl font-semibold">审批详情</h1>
      </div>

      <Card>
        <CardHeader>
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="outline">{resourceTypeLabel(event.resource_type)}</Badge>
            <Badge variant="outline">{eventTypeLabel(event.event_type)}</Badge>
            <Badge variant={APPROVAL_STATUS_BADGE[event.approval_status] ?? 'outline'}>
              {approvalStatusLabel(event.approval_status)}
            </Badge>
            {isPending && remainingVotes !== null && remainingVotes > 0 && (
              <Badge variant="secondary">还差 {remainingVotes} 票</Badge>
            )}
          </div>
          <p className="text-sm text-muted-foreground">
            发起人:<UserName userId={event.actor_user_id} /> · {formatDateTime(event.created_at)}
          </p>
        </CardHeader>
        <CardContent>
          <DiffView oldValue={event.old_value} newValue={event.new_value} />
        </CardContent>
      </Card>

      {isPending && canApprove && <ReviewActions eventId={event.id} />}

      <Card>
        <CardHeader>
          <CardTitle className="text-base">审核历史</CardTitle>
        </CardHeader>
        <CardContent>
          {historyQuery.isLoading ? (
            <p className="text-sm text-muted-foreground">加载中…</p>
          ) : !historyQuery.data || historyQuery.data.length === 0 ? (
            <p className="text-sm text-muted-foreground">暂无审核记录</p>
          ) : (
            <div className="flex flex-col">
              {historyQuery.data.map((review, index) => (
                <div key={review.id}>
                  {index > 0 && <Separator className="my-3" />}
                  <div className="flex flex-col gap-1">
                    <div className="flex items-center gap-2">
                      <Badge variant={review.event_type === 3 ? 'default' : 'destructive'}>
                        {eventTypeLabel(review.event_type)}
                      </Badge>
                      <span className="text-sm">
                        <UserName userId={review.actor_user_id} />
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {formatDateTime(review.created_at)}
                      </span>
                    </div>
                    {review.remark && (
                      <p className="text-sm text-muted-foreground">备注:{review.remark}</p>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

function ReviewActions({ eventId }: { eventId: string }) {
  const [dialog, setDialog] = useState<'approve' | 'reject' | null>(null)
  const [remark, setRemark] = useState('')
  const approveMutation = useReviewEvent('approve')
  const rejectMutation = useReviewEvent('reject')
  const mutation = dialog === 'approve' ? approveMutation : rejectMutation

  const submit = () => {
    if (!dialog) return
    mutation.mutate(
      { eventId, remark: remark.trim() || undefined },
      {
        onSuccess: () => {
          notify.success(dialog === 'approve' ? '已同意' : '已驳回,一票否决已终结该事件')
          setDialog(null)
          setRemark('')
        },
        onError: (error) => notify.error(error),
      },
    )
  }

  return (
    <>
      <div className="flex gap-2">
        <Button className="flex-1" onClick={() => setDialog('approve')}>
          同意
        </Button>
        <Button variant="destructive" className="flex-1" onClick={() => setDialog('reject')}>
          驳回
        </Button>
      </div>

      <Dialog open={dialog !== null} onOpenChange={(open) => !open && setDialog(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{dialog === 'approve' ? '同意该事件?' : '驳回该事件?'}</DialogTitle>
            <DialogDescription>
              {dialog === 'approve'
                ? '达到所需票数后变更将自动生效。'
                : '驳回为一票否决,事件将立即终结。'}
            </DialogDescription>
          </DialogHeader>
          <Textarea
            placeholder="备注(选填)"
            value={remark}
            onChange={(e) => setRemark(e.target.value)}
            maxLength={2000}
          />
          <DialogFooter>
            <Button variant="outline" onClick={() => setDialog(null)}>
              取消
            </Button>
            <Button
              variant={dialog === 'reject' ? 'destructive' : 'default'}
              onClick={submit}
              disabled={mutation.isPending}
            >
              {mutation.isPending ? '提交中…' : '确认'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
