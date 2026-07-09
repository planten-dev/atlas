import { useQuery } from '@tanstack/react-query'
import { customerDetailOptions } from '@/hooks/useCustomers'

function CustomerNameCell({ customerId }: { customerId: string }) {
  const { data } = useQuery({ ...customerDetailOptions(customerId), staleTime: 10 * 60_000 })
  return <span>{data?.name ?? '…'}</span>
}

export function customerNameCell(customerId: string) {
  return <CustomerNameCell customerId={customerId} />
}
