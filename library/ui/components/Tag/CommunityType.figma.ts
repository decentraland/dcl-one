// url=<DCL_FIGMA_FILE>?node-id=9803-3828
// source=components/Tag/Tag.tsx
// component=Tag
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

const variant = instance.getEnum('Property 1', { Public: 'public', Private: 'private' })

export default {
  example: figma.tsx`<Tag variant="${variant}" />`,
  imports: ["import { Tag } from '@dcl/one-ui'"],
  id: 'CommunityType',
  metadata: { nestable: true }
}
