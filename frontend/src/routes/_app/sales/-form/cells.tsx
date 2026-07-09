import { CustomerName } from '@/components/customers/CustomerName'

export function customerNameCell(customerId: string) {
  return <CustomerName customerId={customerId} />
}
