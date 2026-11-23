// First: the HUD modules below must never see this origin's storage.
import './isolateStorage'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { installHudScale } from '@explorer-core/lib/hudScale'
// What a HUD host's page loads before the HUD (react-web's main.tsx): the shared foundations, then its page styles
// with the HUD's tokens.
import '@dcl/one-ui/foundations.css'
import './hud.css'
import { keepLinksInCanvas } from '../frame/links'
import { HudCanvas } from './HudCanvas'

// --ui-scale follows this window, which is the canvas iframe, as the hosts install it before the HUD renders.
installHudScale()
keepLinksInCanvas()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <HudCanvas search={window.location.search} />
  </StrictMode>
)
