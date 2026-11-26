// ContextMenu — the shared ContextMenu. A list it opens beside a row is a cancel layer, so the engine's Cancel
// action (key or gamepad) closes just that list and not the menu around it.

import { ContextMenu as SharedContextMenu, type ContextMenuItem, type ContextMenuProps } from '@dcl/one-ui'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'

export type { ContextMenuItem }

export function ContextMenu(props: Omit<ContextMenuProps, 'whileSubmenuOpen'>): React.JSX.Element {
  return <SharedContextMenu {...props} whileSubmenuOpen={registerCancelLayer} />
}
