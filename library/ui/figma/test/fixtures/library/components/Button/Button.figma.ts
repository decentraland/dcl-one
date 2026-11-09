// url=<DCL_FIGMA_FILE>?node-id=20-1
// source=components/Button/Button.tsx
// component=Button
// contract=1
// states=Hover
import figma from 'figma'
const instance = figma.selectedInstance

const variant = instance.getEnum('Property 1', {
  Primary: 'primary',
  Secondary: 'secondary',
  Terciary: 'tertiary',
  Quaternary: 'quaternary'
})
const disabled = instance.getEnum('Blocked', { Yes: ' disabled', No: '' })
const labelLayer = instance.findText('Label')
const label = labelLayer.type === 'TEXT' ? labelLayer.textContent : ''

export default {
  example: figma.tsx`<Button variant="${variant}"${disabled}>${label}</Button>`,
  imports: ["import { Button } from '@dcl/one-ui'"],
  id: 'button',
  metadata: { nestable: true }
}
