// url=<DCL_FIGMA_FILE>?node-id=11478-1535
// source=components/Tag/Tag.tsx
// component=Tag
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// Coordinates and a World's name are the same tag with another text.
const text = instance.getEnum('Property 1', { Coords: '{coordinates}', World: '{worldName}' })

export default {
  example: figma.tsx`<Tag variant="location">${text}</Tag>`,
  imports: ["import { Tag } from '@dcl/one-ui'"],
  id: 'Location',
  metadata: { nestable: true }
}
