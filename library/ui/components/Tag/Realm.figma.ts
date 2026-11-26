// url=<DCL_FIGMA_FILE>?node-id=71-719
// source=components/Tag/Tag.tsx
// component=Tag
// contract=1
import figma from 'figma'

export default {
  example: figma.tsx`<Tag variant="realm" count={count} countLabel={countLabel}>{realm}</Tag>`,
  imports: ["import { Tag } from '@dcl/one-ui'"],
  id: 'Realm',
  metadata: { nestable: true }
}
