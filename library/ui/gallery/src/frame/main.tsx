import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@dcl/one-ui/foundations.css'
import '../document.css'
import { Canvas } from './Canvas'

// Examples link to fragments or to app paths; only fragments navigate, so the canvas never leaves the gallery.
window.addEventListener('click', (event) => {
  const anchor = event.target instanceof Element ? event.target.closest('a[href]') : null
  if (anchor && !anchor.getAttribute('href')?.startsWith('#')) event.preventDefault()
})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Canvas search={window.location.search} />
  </StrictMode>
)
