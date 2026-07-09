import { createFileRoute, Link, notFound, redirect } from '@tanstack/react-router'
import { Construction } from 'lucide-react'
import { Card, CardContent } from '@/components/ui/card'
import { catalogById, CATALOG } from '@/reports/registry'

export const Route = createFileRoute('/_app/reports/$reportId')({
  beforeLoad: ({ params }) => {
    const item = catalogById(params.reportId)
    if (!item) throw notFound()
    // available:立即跳到真实路由(带预设参数)。
    // redirect 的 href 会进 Headers(仅容 Latin-1),中文参数必须先百分号编码。
    if (item.status === 'available' && item.route) {
      throw redirect({ href: encodeURI(item.route) })
    }
  },
  component: ReportPlaceholderPage,
})

function ReportPlaceholderPage() {
  const { reportId } = Route.useParams()
  const item = catalogById(reportId)
  if (!item) return null

  // 就近可用页引导:同分组的 available 条目
  const nearby = CATALOG.filter((c) => c.group === item.group && c.status === 'available')

  return (
    <div className="mx-auto flex max-w-xl flex-col items-center gap-4 py-16 text-center">
      <Construction className="size-12 text-muted-foreground" />
      <div>
        <h1 className="text-xl font-semibold">{item.title}</h1>
        <p className="mt-1 text-sm text-muted-foreground">{item.group} · 建设中</p>
      </div>
      {item.note && (
        <Card className="w-full">
          <CardContent className="py-3 text-sm text-muted-foreground">{item.note}</CardContent>
        </Card>
      )}
      {nearby.length > 0 && (
        <div className="flex flex-col gap-1 text-sm">
          <p className="text-muted-foreground">您可以先使用:</p>
          {nearby.map((n) => (
            <Link
              key={n.id}
              to="/reports/$reportId"
              params={{ reportId: n.id }}
              preload={false}
              className="text-primary underline underline-offset-4"
            >
              {n.title}
            </Link>
          ))}
        </div>
      )}
      <Link to="/" className="text-sm text-primary underline underline-offset-4">
        回到工作台
      </Link>
    </div>
  )
}
