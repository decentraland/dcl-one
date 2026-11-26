// AnchoredMenu — the shared AnchoredMenu, at the HUD's scale. While open it is a cancel layer, so the engine's
// Cancel action closes it.

import { AnchoredMenu as SharedAnchoredMenu, type AnchoredMenuProps } from '@dcl/one-ui'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { getHudScale } from '@explorer-core/lib/hudScale'

export { anchoredMenuPosition } from '@dcl/one-ui'

export function AnchoredMenu(props: Omit<AnchoredMenuProps, 'scale' | 'whileOpen'>): React.JSX.Element {
  return <SharedAnchoredMenu {...props} scale={getHudScale() || 1} whileOpen={registerCancelLayer} />
}
