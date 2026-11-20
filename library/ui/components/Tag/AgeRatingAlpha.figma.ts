// url=<DCL_FIGMA_FILE>?node-id=8706-8009
// source=components/Tag/Tag.tsx
// component=Tag
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

const variant = instance.getEnum('Flag', { Teen: 'teen', Adult: 'adult', Restricted: 'restricted' })

export default {
  example: figma.tsx`<Tag variant="${variant}" />`,
  imports: ["import { Tag } from '@dcl/one-ui'"],
  id: 'AgeRatingAlpha',
  metadata: { nestable: true }
}
