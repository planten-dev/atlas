import { Users, Network, Store, type LucideIcon } from 'lucide-react'

export interface ManageEntry {
  title: string
  to: string
  icon: LucideIcon
  perm: string
}

/** 移动端"管理"tab 菜单条目;MobileShell 的 TabBar 可见性复用 MANAGE_PERMS。 */
export const MANAGE_ENTRIES: ManageEntry[] = [
  { title: '客户', to: '/customers', icon: Users, perm: 'customers:read' },
  { title: '体系管理', to: '/org/systems', icon: Network, perm: 'systems:read' },
  { title: '门店管理', to: '/org/stores', icon: Store, perm: 'stores:read' },
]

export const MANAGE_PERMS = MANAGE_ENTRIES.map((entry) => entry.perm)

export function visibleManageEntries(permissions: ReadonlySet<string>): ManageEntry[] {
  return MANAGE_ENTRIES.filter((entry) => permissions.has(entry.perm))
}
