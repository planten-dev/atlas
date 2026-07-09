import { http, HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClientProvider, useQuery } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { server } from '@/mocks/server'
import {
  buildDepartmentTree,
  departmentTreeOptions,
  type DepartmentResponse,
} from '@/hooks/useDepartments'
import { createTestQueryClient } from '@/test/utils'

const makeDept = (id: string, name: string, parentId: string | null): DepartmentResponse => ({
  id,
  source: 'dingtalk',
  external_department_id: `ext-${id}`,
  parent_id: parentId,
  name,
  status: 'active',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
})

describe('buildDepartmentTree', () => {
  it('按 parent_id 建树并按名称排序', () => {
    const tree = buildDepartmentTree([
      makeDept('1', '总部', null),
      makeDept('2', '乙部门', '1'),
      makeDept('3', '甲部门', '1'),
      makeDept('4', '孙部门', '3'),
    ])
    expect(tree).toHaveLength(1)
    expect(tree[0]?.children.map((c) => c.name)).toEqual(['甲部门', '乙部门'])
    expect(tree[0]?.children[0]?.children[0]?.name).toBe('孙部门')
  })

  it('孤儿节点(父不在集合内)提升为根', () => {
    const tree = buildDepartmentTree([makeDept('2', '孤儿', 'missing')])
    expect(tree).toHaveLength(1)
  })
})

describe('departmentTreeOptions', () => {
  it('多页数据全部拉取后建树(201 条 → 2 次请求)', async () => {
    const all = Array.from({ length: 201 }, (_, i) => makeDept(`d${i}`, `部门${i}`, null))
    let requests = 0
    server.use(
      http.get('/api/v1/departments/list', ({ request }) => {
        requests += 1
        const url = new URL(request.url)
        const pageNumber = Number(url.searchParams.get('page_number') ?? '1')
        const pageSize = Number(url.searchParams.get('page_size') ?? '50')
        const start = (pageNumber - 1) * pageSize
        return HttpResponse.json({
          departments: all.slice(start, start + pageSize),
          page_number: pageNumber,
          page_size: pageSize,
          total_count: all.length,
        })
      }),
    )

    const queryClient = createTestQueryClient()
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    )
    const { result } = renderHook(() => useQuery(departmentTreeOptions('active')), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(requests).toBe(2)
    expect(result.current.data).toHaveLength(201)
  })
})
