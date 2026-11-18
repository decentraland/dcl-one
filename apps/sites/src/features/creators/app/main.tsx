import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClientProvider } from '@tanstack/react-query'
import { BrowserRouter } from 'react-router-dom'
import { basePath } from '~/config'
import { initAnalytics } from '~/lib/analytics'
import { initSentry } from '~/lib/monitoring'
import { createQueryClient } from '~/lib/queryClient'
import { App } from './App'
import '~/styles/index.css'

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
