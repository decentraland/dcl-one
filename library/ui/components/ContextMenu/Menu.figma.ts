// url=<DCL_FIGMA_FILE>?node-id=16673-641
// source=components/ContextMenu/ContextMenu.tsx
// component=ContextMenu
// contract=1
import figma from 'figma'

// The menu surface: each menu-item in its slot is one of the items, and a width set on the surface is the width prop.
export default {
  example: figma.tsx`<ContextMenu items={items} aria-label={label} />`,
  imports: ["import { ContextMenu } from '@dcl/one-ui'"],
  id: 'bold/menu'
}
