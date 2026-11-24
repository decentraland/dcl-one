import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@dcl/one-ui/foundations.css'
import '../document.css'
import { Shell } from './Shell'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Shell search={window.location.search} />
  </StrictMode>
)
