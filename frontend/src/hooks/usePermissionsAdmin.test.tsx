import { http, HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClientProvider, useQuery } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { server } from '@/mocks/server'
import {
  dedupePolicyItems,
  roleUsersOptions,
  useReplaceSubjectPolicies,
  useSetRoleParents,
  useToggleRoleMember,
  type ReplacePolicyItem,
} from '@/hooks/usePermissionsAdmin'
import { ApiError } from '@/api/error'
import { createTestQueryClient } from '@/test/utils'

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={createTestQueryClient()}>{children}</QueryClientProvider>
}

describe('dedupePolicyItems', () => {
  it('(object, action) 去重,后写覆盖先写', () => {
    const items: ReplacePolicyItem[] = [
      { object: 'products', action: 'read', effect: 'allow' },
      { object: 'products', action: 'write', effect: 'allow' },
      { object: 'products', action: 'read', effect: 'deny' },
    ]
    const result = dedupePolicyItems(items)
    expect(result).toHaveLength(2)
    expect(result.find((i) => i.action === 'read')?.effect).toBe('deny')
  })

  it('空数组原样返回(契约:清空)', () => {
    expect(dedupePolicyItems([])).toEqual([])
  })
})

describe('useReplaceSubjectPolicies', () => {
  it('发送整体替换请求并返回结果', async () => {
    let requestBody: unknown = null
    server.use(
      http.put('/api/v1/permissions/subjects/user/u1/policies', async ({ request }) => {
        requestBody = await request.json()
        return HttpResponse.json({
          subject_kind: 'user',
          subject_id: 'u1',
          policies: [],
        })
      }),
    )
    const { result } = renderHook(() => useReplaceSubjectPolicies(), { wrapper })
    result.current.mutate({
      subjectKind: 'user',
      subjectId: 'u1',
      policies: [{ object: 'products', action: 'read', effect: 'allow' }],
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(requestBody).toEqual({
      policies: [{ object: 'products', action: 'read', effect: 'allow' }],
    })
  })

  it('permission_denied 错误路径', async () => {
    server.use(
      http.put('/api/v1/permissions/subjects/role/r1/policies', () =>
        HttpResponse.json({ error: 'permission_denied', message: 'no' }, { status: 403 }),
      ),
    )
    const { result } = renderHook(() => useReplaceSubjectPolicies(), { wrapper })
    result.current.mutate({ subjectKind: 'role', subjectId: 'r1', policies: [] })
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect((result.current.error as ApiError).code).toBe('permission_denied')
  })
})

describe('roleUsersOptions', () => {
  it('返回角色成员列表(精简字段:id + status)', async () => {
    server.use(
      http.get('/api/v1/permissions/roles/r1/users', () =>
        HttpResponse.json({
          role_id: 'r1',
          users: [{ id: 'u1', status: 'active' }],
        }),
      ),
    )
    const { result } = renderHook(() => useQuery(roleUsersOptions('r1')), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.role_id).toBe('r1')
    expect(result.current.data?.users.map((u) => u.id)).toEqual(['u1'])
  })
})

describe('useToggleRoleMember', () => {
  it('add:PUT 单成员端点,不触碰用户其他角色', async () => {
    let putCalled = false
    server.use(
      http.put('/api/v1/permissions/roles/r1/users/u1', () => {
        putCalled = true
        return new HttpResponse(null, { status: 204 })
      }),
    )
    const { result } = renderHook(() => useToggleRoleMember(), { wrapper })
    result.current.mutate({ roleId: 'r1', userId: 'u1', op: 'add' })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(putCalled).toBe(true)
    expect(result.current.data?.kind).toBe('applied')
  })

  it('remove:DELETE 单成员端点', async () => {
    let deleteCalled = false
    server.use(
      http.delete('/api/v1/permissions/roles/r1/users/u1', () => {
        deleteCalled = true
        return new HttpResponse(null, { status: 204 })
      }),
    )
    const { result } = renderHook(() => useToggleRoleMember(), { wrapper })
    result.current.mutate({ roleId: 'r1', userId: 'u1', op: 'remove' })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(deleteCalled).toBe(true)
  })

  it('protected_system_role 错误路径(移除 super_admin 成员)', async () => {
    server.use(
      http.delete('/api/v1/permissions/roles/r1/users/u1', () =>
        HttpResponse.json({ error: 'protected_system_role', message: 'protected' }, { status: 422 }),
      ),
    )
    const { result } = renderHook(() => useToggleRoleMember(), { wrapper })
    result.current.mutate({ roleId: 'r1', userId: 'u1', op: 'remove' })
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect((result.current.error as ApiError).code).toBe('protected_system_role')
  })
})

describe('useSetRoleParents', () => {
  it('PUT parent_role_ids', async () => {
    let requestBody: unknown = null
    server.use(
      http.put('/api/v1/permissions/roles/r1/parents', async ({ request }) => {
        requestBody = await request.json()
        return HttpResponse.json({
          id: 'r1',
          code: 'a',
          name: 'A',
          kind: 'custom',
          priority: 40,
          parent_role_ids: ['r2'],
          created_at: '2026-01-01T00:00:00Z',
          updated_at: '2026-01-01T00:00:00Z',
        })
      }),
    )
    const { result } = renderHook(() => useSetRoleParents(), { wrapper })
    result.current.mutate({ roleId: 'r1', parentRoleIds: ['r2'] })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(requestBody).toEqual({ parent_role_ids: ['r2'] })
  })
})
