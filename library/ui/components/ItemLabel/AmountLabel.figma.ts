// url=<DCL_FIGMA_FILE>?node-id=10652-4245
// source=components/ItemLabel/ItemLabel.tsx
// component=ItemLabel
// contract=1
import figma from 'figma'

export default {
  example: figma.tsx`<ItemLabel variant="amount">{amount}</ItemLabel>`,
  imports: ["import { ItemLabel } from '@dcl/one-ui'"],
  id: 'AmountLabel',
  metadata: { nestable: true }
}
