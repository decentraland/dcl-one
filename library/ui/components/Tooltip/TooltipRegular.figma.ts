// url=<DCL_FIGMA_FILE>?node-id=16846-53476
// source=components/Tooltip/Tooltip.tsx
// component=Tooltip
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// The regular-weight set: the same tip on an Onix surface with regular text.
const side = instance.getEnum('Property 1', {
  Top: 'top',
  Bottom: 'bottom',
  Right: 'right',
  Left: 'left'
})

export default {
  // The text and the anchor are the caller's; the host sets the Onix fill and regular weight through
  // --tooltip-bg and --tooltip-weight on `tipClassName`.
  example: figma.tsx`<Tooltip label={label} side="${side}" arrow tipClassName={tipClassName}>{children}</Tooltip>`,
  imports: ["import { Tooltip } from '@dcl/one-ui'"],
  id: 'tooltip-regular'
}
