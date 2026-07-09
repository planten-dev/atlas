import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query'
import { client, unwrap } from '@/api/client'
import type { components } from '@/api/types.gen'
import { fetchAllPages } from '@/lib/pagination'

export type DepartmentResponse = components['schemas']['DepartmentResponse']

export interface DepartmentNode extends DepartmentResponse {
  children: DepartmentNode[]
}

/** 全量拉取(分页循环)+ parent_id 建树(设计 §9 DepartmentPicker)。 */
export function buildDepartmentTree(departments: DepartmentResponse[]): DepartmentNode[] {
  const nodes = new Map<string, DepartmentNode>(
    departments.map((d) => [d.id, { ...d, children: [] }]),
  )
  const roots: DepartmentNode[] = []
  for (const node of nodes.values()) {
    const parent = node.parent_id ? nodes.get(node.parent_id) : undefined
    if (parent) {
      parent.children.push(node)
    } else {
      roots.push(node)
    }
  }
  const sortRec = (list: DepartmentNode[]) => {
    list.sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'))
    list.forEach((n) => sortRec(n.children))
  }
  sortRec(roots)
  return roots
}

async function fetchAllDepartments(status?: 'active' | 'disabled') {
  return fetchAllPages(async (pageNumber, pageSize) => {
    const data = unwrap(
      await client.GET('/api/v1/departments/list', {
        params: {
          query: { status_filter: status, page_number: pageNumber, page_size: pageSize },
        },
      }),
    )
    return { items: data.departments, totalCount: data.total_count }
  })
}

export function departmentListOptions(status: 'active' | 'disabled' | undefined = 'active') {
  return queryOptions({
    queryKey: ['departments', 'all', status ?? 'all'],
    queryFn: () => fetchAllDepartments(status),
    staleTime: 10 * 60_000,
  })
}

export function departmentTreeOptions(status: 'active' | 'disabled' | undefined = 'active') {
  return queryOptions({
    queryKey: ['departments', 'tree', status ?? 'all'],
    queryFn: async () => buildDepartmentTree(await fetchAllDepartments(status)),
    staleTime: 10 * 60_000,
  })
}

/** 从钉钉全量同步部门。 */
export function useSyncDepartments() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async () => unwrap(await client.POST('/api/v1/departments/sync/dingtalk')),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['departments'] }),
  })
}
