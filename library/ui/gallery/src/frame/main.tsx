import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@dcl/one-ui/foundations.css'
import '../document.css'
import { Canvas } from './Canvas'
import { keepLinksInCanvas } from './links'

keepLinksInCanvas()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Canvas search={window.location.search} />
  </StrictMode>
)
