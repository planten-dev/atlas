import type { ReactNode } from 'react'
import { render } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { PermissionProvider } from '@/auth/PermissionProvider'

export function createTestQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity },
      mutations: { retry: false },
    },
  })
}

export function renderWithProviders(
  ui: ReactNode,
  {
    permissions = new Set<string>(),
    queryClient = createTestQueryClient(),
  }: { permissions?: Set<string>; queryClient?: QueryClient } = {},
) {
  return {
    queryClient,
    ...render(
      <QueryClientProvider client={queryClient}>
        <PermissionProvider permissions={permissions}>{ui}</PermissionProvider>
      </QueryClientProvider>,
    ),
  }
}
