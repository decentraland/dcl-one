// url=<DCL_FIGMA_FILE>?node-id=15785-43185
// source=components/CloseButton/CloseButton.tsx
// component=CloseButton
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

const variant = instance.getEnum('style', { filled: 'filled', plain: 'plain' })
// Hover is the look the component draws under the pointer.
const state = instance.getEnum('state', { default: '', hover: '' })

export default {
  example: figma.tsx`<CloseButton variant="${variant}" onClick={onClose}${state} />`,
  imports: ["import { CloseButton } from '@dcl/one-ui'"],
  id: 'Close Button',
  metadata: { nestable: true }
}
