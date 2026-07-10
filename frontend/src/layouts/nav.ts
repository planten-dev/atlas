import {
  Home,
  ClipboardCheck,
  ShoppingCart,
  Banknote,
  Syringe,
  Gauge,
  Users,
  Package,
  Network,
  Store,
  UserCog,
  FolderTree,
  ShieldCheck,
  ScrollText,
  type LucideIcon,
} from 'lucide-react'

export interface NavItem {
  title: string
  to: string
  icon: LucideIcon
  /** 缺省为公开(登录即可见) */
  perm?: string
}

export interface NavGroup {
  label: string
  items: NavItem[]
}

/** 桌面侧边栏菜单 = 路由树按权限过滤后的投影(设计 §6)。"业务"区由 registry 驱动(M5)。 */
export const NAV_GROUPS: NavGroup[] = [
  {
    label: '工作台',
    items: [
      { title: '工作台', to: '/', icon: Home },
      { title: '审批中心', to: '/approvals', icon: ClipboardCheck, perm: 'events:read' },
    ],
  },
  {
    label: '销售',
    items: [
      { title: '销售记录', to: '/sales', icon: ShoppingCart, perm: 'sales:records:read' },
      { title: '回款记录', to: '/payments', icon: Banknote, perm: 'sales:records:read' },
      { title: '耗用记录', to: '/usages', icon: Syringe, perm: 'sales:operation-usages:read' },
      { title: '剩余查询', to: '/counts', icon: Gauge, perm: 'sales:records:read' },
    ],
  },
  {
    label: '管理',
    items: [
      { title: '产品与类别', to: '/products', icon: Package, perm: 'products:read' },
      { title: '体系管理', to: '/org/systems', icon: Network, perm: 'systems:read' },
      { title: '门店管理', to: '/org/stores', icon: Store, perm: 'stores:read' },
      { title: '用户', to: '/admin/users', icon: UserCog, perm: 'users:read' },
      { title: '部门', to: '/admin/departments', icon: FolderTree, perm: 'departments:read' },
      { title: '角色与策略', to: '/admin/permissions', icon: ShieldCheck, perm: 'system:permissions:read' },
      { title: '审计日志', to: '/admin/audit', icon: ScrollText, perm: 'events:read' },
      { title: '客户', to: '/customers', icon: Users, perm: 'customers:read' },
    ],
  },
]
