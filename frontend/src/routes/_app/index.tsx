import { createFileRoute, Link } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { PlusSquare, ClipboardCheck, Gauge, Syringe } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { usePermissions, usePermission } from '@/auth/PermissionProvider'
import { useApprovalBadgeCount } from '@/hooks/useApprovalBadge'
import { salesListOptions } from '@/hooks/useSales'
import { customerNameCell } from './sales/-form/cells'
import { formatDate } from '@/lib/date'
import { formatAmount } from '@/lib/money'

export const Route = createFileRoute('/_app/')({
  staticData: { tab: 'home' },
  component: HomePage,
})

const QUICK_LINKS = [
  { title: '销售录入', to: '/sales/new', icon: PlusSquare, perm: 'sales:records:write' },
  { title: '登记耗用', to: '/usages/new', icon: Syringe, perm: 'sales:operation-usages:write' },
  { title: '剩余查询', to: '/counts', icon: Gauge, perm: 'sales:operation-counts:read' },
  { title: '审批中心', to: '/approvals', icon: ClipboardCheck, perm: 'events:read' },
]

function HomePage() {
  const permissions = usePermissions()
  const pendingCount = useApprovalBadgeCount()
  const links = QUICK_LINKS.filter((l) => permissions.has(l.perm))

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      <h1 className="text-xl font-semibold">工作台</h1>

      {permissions.has('events:read') && (
        <Link to="/approvals">
          <Card className="transition-colors hover:bg-accent/50">
            <CardHeader>
              <CardTitle className="text-base">待审批</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-3xl font-bold">{pendingCount}</p>
              <p className="mt-1 text-sm text-muted-foreground">笔事件等待处理</p>
            </CardContent>
          </Card>
        </Link>
      )}

      {links.length > 0 && (
        <div>
          <h2 className="mb-2 text-sm font-medium text-muted-foreground">快捷入口</h2>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            {links.map((link) => (
              <Link key={link.to} to={link.to}>
                <Card className="transition-colors hover:bg-accent/50">
                  <CardContent className="flex flex-col items-center gap-2 py-6">
                    <link.icon className="size-6 text-primary" />
                    <span className="text-sm">{link.title}</span>
                  </CardContent>
                </Card>
              </Link>
            ))}
          </div>
        </div>
      )}

      <RecentSalesCard />
    </div>
  )
}

function RecentSalesCard() {
  const canRead = usePermission('sales:records:read')
  const { data } = useQuery({
    ...salesListOptions({ status_filter: 'active', page_number: 1, page_size: 5 }),
    enabled: canRead,
  })
  if (!canRead) return null

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">最近销售</CardTitle>
      </CardHeader>
      <CardContent>
        {!data || data.items.length === 0 ? (
          <p className="text-sm text-muted-foreground">暂无销售记录</p>
        ) : (
          <div className="flex flex-col divide-y">
            {data.items.map((record) => (
              <Link
                key={record.id}
                to="/sales/$salesId"
                params={{ salesId: record.id }}
                className="flex items-center justify-between gap-2 py-2 text-sm hover:bg-accent/30"
              >
                <span>{customerNameCell(record.customer_id)}</span>
                <span className="text-muted-foreground">{formatDate(record.record_date)}</span>
                <span className="tabular-nums">{formatAmount(record.paid_amount)}</span>
              </Link>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
