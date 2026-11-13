// url=<DCL_FIGMA_FILE>?node-id=15808-46232
// source=components/TabsBar/TabsBar.tsx
// component=TabsBar
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance
const page = instance.getEnum('page', {
  create: ' page="create"',
  shop: ' page="shop" search={searchControl} actions={actions}',
  learn: ' page="learn" search={searchControl} actions={actions}'
})
// The viewport selects responsive CSS; the application supplies real controls and route data.
const viewport = instance.getEnum('viewport', { desktop: '', mobile: '', 'large-desktop': '' })
export default {
  example: figma.tsx`<TabsBar${page}${viewport} items={items} activeId={activeId} label="Sections" />`,
  imports: ["import { TabsBar } from '@dcl/one-ui'"],
  id: 'TabsBar',
  metadata: { nestable: true }
}
