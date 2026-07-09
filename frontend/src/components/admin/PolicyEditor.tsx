import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Plus, Trash2 } from 'lucide-react'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
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
import {
  catalogOptions,
  dedupePolicyItems,
  policiesListOptions,
  useReplaceSubjectPolicies,
  type ReplacePolicyItem,
} from '@/hooks/usePermissionsAdmin'
import { POLICY_EFFECT_LABELS } from '@/lib/labels'
import { notify } from '@/lib/notify'

const ACTIONS: ReplacePolicyItem['action'][] = ['read', 'write', 'approve', '*']

/**
 * 主体(用户/角色)策略编辑器:行按 catalog 构建 object/action/effect;
 * 保存 = PUT 整体替换(空列表即清空),对话框提示覆盖语义。
 */
export function PolicyEditor({
  subjectKind,
  subjectId,
}: {
  subjectKind: 'user' | 'role'
  subjectId: string
}) {
  const { data: catalog } = useQuery(catalogOptions)
  const currentQuery = useQuery(
    policiesListOptions({ subject_kind: subjectKind, subject_id: subjectId }),
  )
  const replaceMutation = useReplaceSubjectPolicies()

  const [draft, setDraft] = useState<ReplacePolicyItem[] | null>(null)
  const rows: ReplacePolicyItem[] =
    draft ??
    (currentQuery.data ?? []).map((p) => ({
      object: p.object,
      action: p.action as ReplacePolicyItem['action'],
      effect: p.effect,
    }))

  const objectOptions = useMemo(() => {
    const objects = (catalog ?? []).map((entry) => ({
      value: entry.object,
      label: `${entry.label}(${entry.object})`,
    }))
    return [...objects, { value: '*', label: '全部资源(*)' }]
  }, [catalog])

  const dirty = draft !== null

  const update = (index: number, patch: Partial<ReplacePolicyItem>) => {
    setDraft(rows.map((row, i) => (i === index ? { ...row, ...patch } : row)))
  }

  return (
    <div className="flex flex-col gap-3">
      {currentQuery.isLoading ? (
        <p className="text-sm text-muted-foreground">加载中…</p>
      ) : rows.length === 0 ? (
        <p className="text-sm text-muted-foreground">暂无个性化策略</p>
      ) : (
        <div className="flex flex-col gap-2">
          {rows.map((row, index) => (
            <div key={index} className="flex flex-wrap items-center gap-2">
              <Select
                value={row.object || null}
                onValueChange={(value) => value && update(index, { object: value })}
              >
                <SelectTrigger size="sm" className="min-w-56 flex-1">
                  <SelectValue placeholder="选择资源" />
                </SelectTrigger>
                <SelectContent>
                  {objectOptions.map((opt) => (
                    <SelectItem key={opt.value} value={opt.value}>
                      {opt.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Select
                value={row.action}
                onValueChange={(value) =>
                  value && update(index, { action: value as ReplacePolicyItem['action'] })
                }
              >
                <SelectTrigger size="sm" className="w-28">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {ACTIONS.map((action) => (
                    <SelectItem key={action} value={action}>
                      {action}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Select
                value={row.effect}
                onValueChange={(value) =>
                  value && update(index, { effect: value as 'allow' | 'deny' })
                }
              >
                <SelectTrigger size="sm" className="w-24">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="allow">{POLICY_EFFECT_LABELS.allow}</SelectItem>
                  <SelectItem value="deny">{POLICY_EFFECT_LABELS.deny}</SelectItem>
                </SelectContent>
              </Select>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => setDraft(rows.filter((_, i) => i !== index))}
                aria-label="删除策略行"
              >
                <Trash2 />
              </Button>
            </div>
          ))}
        </div>
      )}

      <div className="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          onClick={() => setDraft([...rows, { object: '', action: 'read', effect: 'allow' }])}
        >
          <Plus />
          添加策略
        </Button>
        {dirty && (
          <>
            <AlertDialog>
              <AlertDialogTrigger render={<Button size="sm" />}>保存</AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>保存策略?</AlertDialogTitle>
                  <AlertDialogDescription>
                    保存将整体覆盖该{subjectKind === 'user' ? '用户' : '角色'}
                    的全部个性化策略(共 {dedupePolicyItems(rows.filter((r) => r.object)).length} 条)。
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>取消</AlertDialogCancel>
                  <AlertDialogAction
                    onClick={() => {
                      const policies = dedupePolicyItems(rows.filter((r) => r.object))
                      replaceMutation.mutate(
                        { subjectKind, subjectId, policies },
                        {
                          onSuccess: () => {
                            notify.success('策略已保存')
                            setDraft(null)
                          },
                          onError: (error) => notify.error(error),
                        },
                      )
                    }}
                  >
                    确认保存
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
            <Button variant="ghost" size="sm" onClick={() => setDraft(null)}>
              放弃修改
            </Button>
            <Badge variant="secondary">未保存</Badge>
          </>
        )}
      </div>
    </div>
  )
}
