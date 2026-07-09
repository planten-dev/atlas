import { createFileRoute } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { RefreshCw } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Guard } from '@/auth/PermissionProvider'
import { requirePerm } from '@/auth/route-guard'
import {
  departmentTreeOptions,
  useSyncDepartments,
  type DepartmentNode,
} from '@/hooks/useDepartments'
import { notify } from '@/lib/notify'

export const Route = createFileRoute('/_app/admin/departments')({
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'departments:read'),
  staticData: { desktopOnly: true },
  component: DepartmentsPage,
})

function DepartmentTreeNode({ node, depth }: { node: DepartmentNode; depth: number }) {
  return (
    <>
      <div
        className="flex items-center gap-2 py-1.5 text-sm"
        style={{ paddingLeft: `${depth * 1.25}rem` }}
      >
        <span>{node.name}</span>
        {node.status === 'disabled' && <Badge variant="destructive">停用</Badge>}
        {node.source === 'manual' && <Badge variant="secondary">手工</Badge>}
      </div>
      {node.children.map((child) => (
        <DepartmentTreeNode key={child.id} node={child} depth={depth + 1} />
      ))}
    </>
  )
}

function DepartmentsPage() {
  // 管理页展示全部状态(含停用)
  const { data: tree, isLoading } = useQuery(departmentTreeOptions(undefined))
  const syncMutation = useSyncDepartments()

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">部门</h1>
        <Guard perm="departments:write">
          <Button
            variant="outline"
            disabled={syncMutation.isPending}
            onClick={() => {
              syncMutation.mutate(undefined, {
                onSuccess: (result) => {
                  notify.success(
                    `同步完成:新增 ${result.created},更新 ${result.updated},未变化 ${result.unchanged}`,
                  )
                },
                onError: (error) => notify.error(error),
              })
            }}
          >
            <RefreshCw className={syncMutation.isPending ? 'animate-spin' : undefined} />
            从钉钉同步
          </Button>
        </Guard>
      </div>

      <p className="text-sm text-muted-foreground">
        部门以钉钉为准,此处只读;结构变化请在钉钉调整后同步。
      </p>

      <Card>
        <CardContent className="py-3">
          {isLoading ? (
            <p className="text-sm text-muted-foreground">加载中…</p>
          ) : !tree || tree.length === 0 ? (
            <p className="text-sm text-muted-foreground">暂无部门,请先从钉钉同步</p>
          ) : (
            tree.map((node) => <DepartmentTreeNode key={node.id} node={node} depth={0} />)
          )}
        </CardContent>
      </Card>
    </div>
  )
}
