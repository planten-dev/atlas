/** 循环拉取分页接口的全部数据(部门/人员选择器等小规模全量场景)。 */
export async function fetchAllPages<T>(
  fetchPage: (pageNumber: number, pageSize: number) => Promise<{ items: T[]; totalCount: number }>,
  pageSize = 200,
): Promise<T[]> {
  const all: T[] = []
  let pageNumber = 1
  for (;;) {
    const { items, totalCount } = await fetchPage(pageNumber, pageSize)
    all.push(...items)
    if (all.length >= totalCount || items.length === 0) break
    pageNumber += 1
  }
  return all
}
