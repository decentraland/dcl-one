// url=<DCL_FIGMA_FILE>?node-id=15843-56922
// source=components/TitleBar/TitleBar.tsx
// component=TitleBar
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance
const variant = instance.getEnum('Property 1', {
  Backpack: ' variant="backpack" controls={controls}',
  Settings: ' variant="settings"',
  Events: ' variant="events" controls={controls}',
  Places: ' variant="places" controls={controls}',
  SHOP: ' variant="shop" controls={controls}'
})
export default {
  example: figma.tsx`<TitleBar${variant} sections={sections} activeId={activeId} onSelect={onSelect} />`,
  imports: ["import { TitleBar } from '@dcl/one-ui'"],
  id: 'TitleBar',
  metadata: { nestable: true }
}
