import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider, createRouter } from '@tanstack/react-router'
import '@fontsource-variable/inter'
import './index.css'
import { routeTree } from './routeTree.gen'
import { setUnauthorizedHandler } from '@/api/client'
import { Toaster } from '@/components/ui/sonner'
import { prepareBrowserCompatibility, renderUnsupportedBrowser } from '@/compat/browser'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 30_000,
    },
  },
})

const router = createRouter({
  routeTree,
  context: { queryClient },
  defaultPreload: 'intent',
})

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

// 401 全局拦截:清 Query 缓存 → 跳登录页并带上原路径(设计 §4)。
setUnauthorizedHandler((currentPath) => {
  queryClient.clear()
  void router.navigate({
    to: '/login',
    search: currentPath && currentPath !== '/' ? { redirect: currentPath } : {},
  })
})

async function bootstrap() {
  const rootElement = document.getElementById('root')
  if (!rootElement) throw new Error('Missing #root element')

  const compatibility = await prepareBrowserCompatibility()
  if (!compatibility.supported) {
    renderUnsupportedBrowser(rootElement)
    return
  }

  if (import.meta.env.VITE_ENABLE_MOCKS === 'true') {
    const { worker } = await import('./mocks/browser')
    await worker.start({ onUnhandledRequest: 'bypass' })
  }

  createRoot(rootElement).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
        <Toaster position="top-center" richColors />
      </QueryClientProvider>
    </StrictMode>,
  )
}

void bootstrap()
