// url=<DCL_FIGMA_FILE>?node-id=16673-785
// source=components/ContextMenu/ContextMenu.tsx
// component=ContextMenu
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// A composed menu: its rows are the items, and the chat type's open submenu is the items of its submenu row. The
// user type opens with the profile card's header (menu-header), which this component does not draw.
const type = instance.getEnum('type', { chat: '' })

export default {
  example: figma.tsx`<ContextMenu items={items} aria-label={label}${type} />`,
  imports: ["import { ContextMenu } from '@dcl/one-ui'"],
  id: 'bold/context-menu'
}
