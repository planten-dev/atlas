import { useQuery } from '@tanstack/react-query'
import { customerDetailOptions } from '@/hooks/useCustomers'

/** customer_id → 客户姓名(长缓存)。 */
export function CustomerName({ customerId }: { customerId: string }) {
  const { data } = useQuery({ ...customerDetailOptions(customerId), staleTime: 10 * 60_000 })
  return <span>{data?.name ?? '…'}</span>
}
