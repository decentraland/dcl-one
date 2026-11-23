// url=<DCL_FIGMA_FILE>?node-id=15841-55389
// source=components/CloseButton/CloseButton.tsx
// component=CloseButton
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// Hover is the look the component draws under the pointer.
const state = instance.getEnum('Property 1', { Default: '', Hover: '' })

export default {
  example: figma.tsx`<CloseButton onClick={onClose}${state} />`,
  imports: ["import { CloseButton } from '@dcl/one-ui'"],
  id: 'CloseCTA',
  metadata: { nestable: true }
}
