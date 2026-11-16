// url=<DCL_FIGMA_FILE>?node-id=15808-46232
// source=components/TabsBar/TabsBar.tsx
// component=TabsBar
// contract=1
// states=viewport
import figma from 'figma'
const instance = figma.selectedInstance
const page = instance.getEnum('page', {
  create: ' page="create"',
  shop: ' page="shop" search={searchControl} actions={actions}',
  learn: ' page="learn" search={searchControl} actions={actions}'
})
// The viewport selects responsive CSS; the application supplies real controls and route data.
export default {
  example: figma.tsx`<TabsBar mode="navigation"${page} items={items} activeId={activeId} label="Sections" />`,
  imports: ["import { TabsBar } from '@dcl/one-ui'"],
  id: 'tabs-bar',
  metadata: { nestable: true }
}
