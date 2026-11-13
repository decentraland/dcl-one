// url=<DCL_FIGMA_FILE>?node-id=15674-34262
// source=components/Tabs/Tabs.tsx
// component=Tabs
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance
const selected = instance.getEnum('Status', {
  Default: '',
  Hover: '',
  Selected: ' selected'
})
// Device selection is owned by responsive CSS, not a forced viewport prop.
const device = instance.getEnum('Device', { Desktop: '', Mobile: '' })
export default {
  example: figma.tsx`<Tabs href={href} label={label}${selected}${device} />`,
  imports: ["import { Tabs } from '@dcl/one-ui'"],
  id: 'Tabs',
  metadata: { nestable: true }
}
