import { useEffect } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { Construction } from 'lucide-react'
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import { Badge } from '@/components/ui/badge'
import { usePermissions } from '@/auth/PermissionProvider'
import { CATALOG_GROUPS, visibleCatalog } from '@/reports/registry'
import { NAV_GROUPS } from '@/layouts/nav'

/** Cmd/Ctrl+K 命令面板:导航 + 业务目录(registry 驱动,旧名可搜)。open 状态由外部持有,便于头部搜索按钮触发。 */
export function CommandPalette({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const navigate = useNavigate()
  const permissions = usePermissions()
  const setOpen = onOpenChange

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        setOpen(!open)
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [open, setOpen])

  const catalog = visibleCatalog(permissions)
  const go = (to: string) => {
    setOpen(false)
    void navigate({ href: to })
  }

  return (
    <CommandDialog open={open} onOpenChange={setOpen} title="搜索" description="搜索页面与业务表单">
      {/* CommandDialog 只是 Dialog 壳,cmdk 的 context 需要 Command 根组件提供 */}
      <Command>
        <CommandInput placeholder="搜索页面或业务表单(旧平台名称可搜)…" />
      <CommandList>
        <CommandEmpty>无匹配结果</CommandEmpty>
        {NAV_GROUPS.map((group) => {
          const items = group.items.filter((item) => !item.perm || permissions.has(item.perm))
          if (items.length === 0) return null
          return (
            <CommandGroup key={group.label} heading={group.label}>
              {items.map((item) => (
                <CommandItem key={item.to} value={`nav-${item.to}`} keywords={[item.title]} onSelect={() => go(item.to)}>
                  <item.icon />
                  {item.title}
                </CommandItem>
              ))}
            </CommandGroup>
          )
        })}
        {CATALOG_GROUPS.map((groupName) => {
          const items = catalog.filter((item) => item.group === groupName)
          if (items.length === 0) return null
          return (
            <CommandGroup key={groupName} heading={`业务 · ${groupName}`}>
              {items.map((item) => (
                <CommandItem
                  key={item.id}
                  value={`report-${item.id}`}
                  keywords={[item.title]}
                  onSelect={() => go(`/reports/${item.id}`)}
                  className={item.status === 'reserved' ? 'opacity-60' : undefined}
                >
                  {item.status === 'reserved' && <Construction />}
                  {item.title}
                  {item.status === 'reserved' && (
                    <Badge variant="secondary" className="ml-auto">
                      建设中
                    </Badge>
                  )}
                </CommandItem>
              ))}
            </CommandGroup>
          )
        })}
        </CommandList>
      </Command>
    </CommandDialog>
  )
}
