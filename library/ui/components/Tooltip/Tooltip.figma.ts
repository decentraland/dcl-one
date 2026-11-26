// url=<DCL_FIGMA_FILE>?node-id=17504-29371
// source=components/Tooltip/Tooltip.tsx
// component=Tooltip
// contract=1
import figma from 'figma'
const instance = figma.selectedInstance

// The peak is the arrow; the variant names the side of the anchor the tip sits on.
const side = instance.getEnum('Property 1', {
  Top: 'top',
  Bottom: 'bottom',
  Right: 'right',
  Left: 'left'
})

export default {
  // The text and the anchor are the caller's. The design's surface (Shadow fill, 12px radius, 16 by 20px padding,
  // 14px strong text) is not the component default: the host sets it through the --tooltip-* properties.
  example: figma.tsx`<Tooltip label={label} side="${side}" arrow>{children}</Tooltip>`,
  imports: ["import { Tooltip } from '@dcl/one-ui'"],
  id: 'tooltip'
}
