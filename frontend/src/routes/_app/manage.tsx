import { createFileRoute, Link } from '@tanstack/react-router'
import { ChevronRight } from 'lucide-react'
import { Card, CardContent } from '@/components/ui/card'
import { usePermissions } from '@/auth/PermissionProvider'
import { visibleManageEntries } from '@/layouts/manage-menu'

/** 移动端管理菜单(TabBar 第 5 tab);条目按权限过滤,无需路由级守卫。 */
export const Route = createFileRoute('/_app/manage')({
  staticData: { tab: 'admin' },
  component: ManagePage,
})

function ManagePage() {
  const permissions = usePermissions()
  const entries = visibleManageEntries(permissions)

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4">
      <h1 className="text-xl font-semibold">管理</h1>
      {entries.length === 0 ? (
        <p className="py-16 text-center text-sm text-muted-foreground">暂无可用的管理功能</p>
      ) : (
        <Card className="py-0">
          <CardContent className="flex flex-col divide-y p-0">
            {entries.map((entry) => (
              <Link
                key={entry.to}
                to={entry.to}
                className="flex items-center gap-3 px-4 py-3.5 text-sm transition-colors hover:bg-accent/50"
              >
                <entry.icon className="size-5 text-primary" />
                <span className="flex-1">{entry.title}</span>
                <ChevronRight className="size-4 text-muted-foreground" />
              </Link>
            ))}
          </CardContent>
        </Card>
      )}
    </div>
  )
}
