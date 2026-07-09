import { useState, type ReactNode } from 'react'
import { Link, useMatches, useRouterState } from '@tanstack/react-router'
import {
  Home,
  ShoppingCart,
  ClipboardCheck,
  User,
  Search,
  Construction,
  ChevronRight,
} from 'lucide-react'
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from '@/components/ui/sidebar'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import { useIsMobile } from '@/hooks/use-mobile'
import { usePermissions } from '@/auth/PermissionProvider'
import { useApprovalBadgeCount } from '@/hooks/useApprovalBadge'
import { NAV_GROUPS } from '@/layouts/nav'
import { CommandPalette } from '@/components/CommandPalette'
import { UserMenu } from '@/components/UserMenu'
import { CATALOG_GROUPS, visibleCatalog } from '@/reports/registry'

export function AppShell({ children }: { children: ReactNode }) {
  const isMobile = useIsMobile()
  if (isMobile) {
    return <MobileShell>{children}</MobileShell>
  }
  return <DesktopShell>{children}</DesktopShell>
}

const SIDEBAR_COLLAPSED_KEY = 'atlas.sidebar.collapsed-groups'

function loadCollapsedGroups(): Set<string> {
  try {
    const raw = localStorage.getItem(SIDEBAR_COLLAPSED_KEY)
    const parsed: unknown = raw ? JSON.parse(raw) : []
    return new Set(Array.isArray(parsed) ? parsed.filter((v) => typeof v === 'string') : [])
  } catch {
    return new Set()
  }
}

/** 可折叠的侧边栏分组:点击组标题收起/展开,折叠状态存 localStorage。 */
function CollapsibleSidebarGroup({ label, children }: { label: string; children: ReactNode }) {
  const [open, setOpen] = useState(() => !loadCollapsedGroups().has(label))

  const handleOpenChange = (next: boolean) => {
    setOpen(next)
    try {
      const collapsed = loadCollapsedGroups()
      if (next) {
        collapsed.delete(label)
      } else {
        collapsed.add(label)
      }
      localStorage.setItem(SIDEBAR_COLLAPSED_KEY, JSON.stringify([...collapsed]))
    } catch {
      // 隐私模式等写入失败:仅本次会话生效
    }
  }

  return (
    <Collapsible open={open} onOpenChange={handleOpenChange}>
      <SidebarGroup className="py-1">
        <SidebarGroupLabel
          render={
            <CollapsibleTrigger className="group/collapsible w-full cursor-pointer hover:bg-sidebar-accent hover:text-sidebar-accent-foreground" />
          }
        >
          <ChevronRight
            className={cn('mr-1 transition-transform duration-200', open && 'rotate-90')}
          />
          {label}
        </SidebarGroupLabel>
        <CollapsibleContent>
          <SidebarGroupContent>{children}</SidebarGroupContent>
        </CollapsibleContent>
      </SidebarGroup>
    </Collapsible>
  )
}

function DesktopShell({ children }: { children: ReactNode }) {
  const permissions = usePermissions()
  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const [paletteOpen, setPaletteOpen] = useState(false)

  return (
    <SidebarProvider>
      <Sidebar>
        <SidebarHeader>
          <Link to="/" className="flex items-center gap-2 px-2 py-1.5">
            <div className="flex size-8 items-center justify-center rounded-lg bg-primary font-bold text-primary-foreground">
              A
            </div>
            <span className="font-semibold">Atlas</span>
          </Link>
        </SidebarHeader>
        <SidebarContent>
          {NAV_GROUPS.map((group) => {
            const visible = group.items.filter((item) => !item.perm || permissions.has(item.perm))
            if (visible.length === 0) return null
            return (
              <CollapsibleSidebarGroup key={group.label} label={group.label}>
                <SidebarMenu>
                  {visible.map((item) => (
                    <SidebarMenuItem key={item.to}>
                      <SidebarMenuButton
                        isActive={
                          item.to === '/' ? pathname === '/' : pathname.startsWith(item.to)
                        }
                        render={
                          <Link to={item.to}>
                            <item.icon />
                            <span>{item.title}</span>
                          </Link>
                        }
                      />
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </CollapsibleSidebarGroup>
            )
          })}
          <BusinessCatalogGroups />
        </SidebarContent>
      </Sidebar>
      <SidebarInset>
        <header className="flex h-12 items-center gap-3 border-b px-4">
          <SidebarTrigger />
          <button
            type="button"
            onClick={() => setPaletteOpen(true)}
            className="ml-auto flex items-center gap-1 rounded-md border px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted"
          >
            <Search className="size-3.5" />
            搜索
            <kbd className="rounded border bg-muted px-1 font-mono text-[10px]">Ctrl K</kbd>
          </button>
          <UserMenu />
        </header>
        <div className="flex-1 p-4 md:p-6">{children}</div>
      </SidebarInset>
      <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} />
    </SidebarProvider>
  )
}

/** 侧边栏"业务"区:registry 按权限过滤后的投影;reserved 置灰 + 建设中徽标(设计 §8)。 */
function BusinessCatalogGroups() {
  const permissions = usePermissions()
  const catalog = visibleCatalog(permissions)

  return (
    <>
      {CATALOG_GROUPS.map((groupName) => {
        const items = catalog.filter((item) => item.group === groupName)
        if (items.length === 0) return null
        return (
          <CollapsibleSidebarGroup key={groupName} label={`业务 · ${groupName}`}>
            <SidebarMenu>
              {items.map((item) => (
                <SidebarMenuItem key={item.id}>
                  <SidebarMenuButton
                    className={item.status === 'reserved' ? 'opacity-60' : undefined}
                    render={
                      // preload=false:该路由 beforeLoad 会抛 redirect,
                      // intent 预加载会在悬停时反复触发重定向导致卡死
                      <Link
                        to="/reports/$reportId"
                        params={{ reportId: item.id }}
                        preload={false}
                      >
                        {item.status === 'reserved' && <Construction />}
                        <span>{item.title}</span>
                      </Link>
                    }
                  />
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </CollapsibleSidebarGroup>
        )
      })}
    </>
  )
}

const MOBILE_TABS = [
  { title: '首页', to: '/', icon: Home, tab: 'home' as const },
  { title: '销售', to: '/sales', icon: ShoppingCart, tab: 'sales' as const },
  { title: '审批', to: '/approvals', icon: ClipboardCheck, tab: 'approvals' as const },
  { title: '我的', to: '/me', icon: User, tab: 'me' as const },
]

function MobileShell({ children }: { children: ReactNode }) {
  const matches = useMatches()
  // 只有标记了 staticData.tab 的路由显示 TabBar;二级页隐藏(设计 §7)
  const activeTab = matches.reduce<string | undefined>(
    (acc, m) => m.staticData.tab ?? acc,
    undefined,
  )
  const desktopOnly = matches.some((m) => m.staticData.desktopOnly)
  const badgeCount = useApprovalBadgeCount()

  return (
    <div className="flex min-h-svh flex-col">
      <div className={cn('flex-1 p-4', activeTab && 'pb-20')}>
        {desktopOnly ? <DesktopOnlyNotice /> : children}
      </div>
      {activeTab && (
        <nav className="fixed inset-x-0 bottom-0 z-40 flex border-t bg-background pb-[env(safe-area-inset-bottom)]">
          {MOBILE_TABS.map((tab) => (
            <Link
              key={tab.to}
              to={tab.to}
              className={cn(
                'relative flex flex-1 flex-col items-center gap-0.5 py-2 text-xs',
                activeTab === tab.tab ? 'text-primary' : 'text-muted-foreground',
              )}
            >
              <tab.icon className="size-5" />
              <span>{tab.title}</span>
              {tab.tab === 'approvals' && badgeCount > 0 && (
                <Badge
                  variant="destructive"
                  className="absolute right-[calc(50%-1.75rem)] top-0.5 h-4 min-w-4 px-1 text-[10px]"
                >
                  {badgeCount > 99 ? '99+' : badgeCount}
                </Badge>
              )}
            </Link>
          ))}
        </nav>
      )}
    </div>
  )
}

/** 桌面专属页在移动端的替代提示。 */
export function DesktopOnlyNotice() {
  return (
    <div className="flex flex-col items-center gap-2 py-16 text-center">
      <p className="font-medium">该功能请在电脑端使用</p>
      <p className="text-sm text-muted-foreground">手机端仅提供审批、销售、耗用与查询功能</p>
    </div>
  )
}
