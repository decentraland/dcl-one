// KebabMenu — the shared KebabMenu. While open it is a cancel layer, so the engine's Cancel action closes it.

import { KebabMenu as SharedKebabMenu, type KebabMenuProps } from '@dcl/one-ui'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'

export function KebabMenu(props: Omit<KebabMenuProps, 'whileOpen'>): React.JSX.Element {
  return <SharedKebabMenu {...props} whileOpen={registerCancelLayer} />
}
