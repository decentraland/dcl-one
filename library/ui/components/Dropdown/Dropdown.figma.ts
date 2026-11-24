// url=<DCL_FIGMA_FILE>?node-id=8711-15094
// source=components/Dropdown/Dropdown.tsx
// component=Dropdown
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// Opened is the look the component draws when the reader opens it.
const state = instance.getEnum('Property 1', { Idle: '', Opened: '', Blocked: ' disabled' })

export default {
  example: figma.tsx`<Dropdown label={label} options={options} value={value} onChange={onChange}${state} />`,
  imports: ["import { Dropdown } from '@dcl/one-ui'"],
  id: 'dropdown'
}
