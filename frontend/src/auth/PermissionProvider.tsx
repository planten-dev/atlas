import { createContext, useContext, type ReactNode } from 'react'

const PermissionContext = createContext<Set<string>>(new Set())

export function PermissionProvider({
  permissions,
  children,
}: {
  permissions: Set<string>
  children: ReactNode
}) {
  return <PermissionContext.Provider value={permissions}>{children}</PermissionContext.Provider>
}

export function usePermissions(): Set<string> {
  return useContext(PermissionContext)
}

export function usePermission(perm: string): boolean {
  return usePermissions().has(perm)
}

/** 有权限才渲染 children;前端只做 UI 隐藏,真实校验在后端。 */
export function Guard({ perm, children }: { perm: string; children: ReactNode }) {
  const allowed = usePermission(perm)
  if (!allowed) return null
  return <>{children}</>
}
