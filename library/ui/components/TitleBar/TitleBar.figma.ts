// url=<DCL_FIGMA_FILE>?node-id=17488-18659
// source=components/TitleBar/TitleBar.tsx
// component=TitleBar
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance
const variant = instance.getEnum('section', {
  backpack: ' variant="backpack" controls={controls}',
  settings: ' variant="settings"',
  events: ' variant="events" controls={controls}',
  places: ' variant="places" controls={controls}',
  shop: ' variant="shop" controls={controls}'
})
export default {
  example: figma.tsx`<TitleBar${variant} sections={sections} activeId={activeId} onSelect={onSelect} />`,
  imports: ["import { TitleBar } from '@dcl/one-ui'"],
  id: 'TitleBar',
  metadata: { nestable: true }
}
