import { describe, expect, it } from 'vitest'
import { screen } from '@testing-library/react'
import { Guard, usePermission } from '@/auth/PermissionProvider'
import { renderWithProviders } from '@/test/utils'

function Probe({ perm }: { perm: string }) {
  const allowed = usePermission(perm)
  return <span>{allowed ? 'yes' : 'no'}</span>
}

describe('PermissionProvider', () => {
  it('usePermission 命中权限集', () => {
    renderWithProviders(<Probe perm="products:read" />, {
      permissions: new Set(['products:read']),
    })
    expect(screen.getByText('yes')).toBeInTheDocument()
  })

  it('usePermission 缺权限返回 false', () => {
    renderWithProviders(<Probe perm="products:write" />, {
      permissions: new Set(['products:read']),
    })
    expect(screen.getByText('no')).toBeInTheDocument()
  })

  it('Guard 有权限时渲染 children', () => {
    renderWithProviders(
      <Guard perm="products:write">
        <button>新建</button>
      </Guard>,
      { permissions: new Set(['products:write']) },
    )
    expect(screen.getByRole('button', { name: '新建' })).toBeInTheDocument()
  })

  it('Guard 无权限时不渲染', () => {
    renderWithProviders(
      <Guard perm="products:write">
        <button>新建</button>
      </Guard>,
      { permissions: new Set() },
    )
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })
})
