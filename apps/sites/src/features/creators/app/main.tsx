import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClientProvider } from '@tanstack/react-query'
import { BrowserRouter } from 'react-router-dom'
import { basePath } from '@/data/lib/creators/config'
import { initAnalytics } from '@/data/lib/creators/analytics'
import { initSentry } from '@/data/lib/creators/monitoring'
import { createQueryClient } from '@/data/lib/creators/queryClient'
import { App } from './App'
import '@/features/creators/styles/index.css'

initSentry()
initAnalytics()

const queryClient = createQueryClient()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter basename={basePath}>
        <App />
      </BrowserRouter>
    </QueryClientProvider>
  </StrictMode>
)
