// url=<DCL_FIGMA_FILE>?node-id=16719-607
// source=components/Checkbox/Checkbox.tsx
// component=Checkbox
// contract=1
// states=viewport
import figma from 'figma'
const instance = figma.selectedInstance
const variant = instance.getEnum('style', {
  outline: '',
  filled: ' variant="filled"'
})
const checked = instance.getEnum('checked', {
  false: '',
  true: ' defaultChecked',
  indeterminate: ' indeterminate'
})
const state = instance.getEnum('state', {
  default: '',
  hover: '',
  pressed: '',
  disabled: ' disabled'
})
const size = instance.getEnum('size', {
  sm: ' size="sm"',
  md: '',
  lg: ' size="lg"'
})
export default {
  example: figma.tsx`<Checkbox aria-label={label}${variant}${size}${checked}${state} />`,
  imports: ["import { Checkbox } from '@dcl/one-ui'"],
  id: 'checkbox',
  metadata: { nestable: true }
}
