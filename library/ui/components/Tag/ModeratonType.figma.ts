// url=<DCL_FIGMA_FILE>?node-id=9803-3823
// source=components/Tag/Tag.tsx
// component=Tag
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// The set's name is spelt this way in the design.
const text = instance.getEnum('Property 1', { Owner: 'Owner', Moderator: 'Moderator' })

export default {
  example: figma.tsx`<Tag variant="role">${text}</Tag>`,
  imports: ["import { Tag } from '@dcl/one-ui'"],
  id: 'ModeratonType',
  metadata: { nestable: true }
}
