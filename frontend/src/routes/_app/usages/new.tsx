import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { z } from 'zod'
import { UsageForm } from '@/components/usages/UsageForm'
import { requirePerm } from '@/auth/route-guard'

const searchSchema = z.object({
  salesRecordId: z.string().optional(),
  // 类别预设(业务目录 ?category= 入口):后端缺按类别过滤前仅作直达入口
  category: z.string().optional(),
})

/** 独立页保留:移动端入口、业务目录深链(?salesRecordId= 预选)。桌面列表页走弹窗。 */
export const Route = createFileRoute('/_app/usages/new')({
  validateSearch: searchSchema,
  beforeLoad: ({ context }) => requirePerm(context.queryClient, 'sales:operation-usages:write'),
  component: UsageNewPage,
})

function UsageNewPage() {
  const search = Route.useSearch()
  const navigate = useNavigate()

  return (
    <div className="mx-auto flex max-w-xl flex-col gap-4">
      <h1 className="text-xl font-semibold">登记耗用</h1>
      <UsageForm
        lockedSalesRecordId={search.salesRecordId}
        onSuccess={(salesRecordId) => {
          void navigate({ to: '/sales/$salesId', params: { salesId: salesRecordId } })
        }}
      />
    </div>
  )
}
