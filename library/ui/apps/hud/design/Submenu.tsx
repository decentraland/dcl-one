// Submenu — the shared Submenu. Its open list is a cancel layer, so one press of Cancel closes the list, not the
// menu it sits in.

import { Submenu as SharedSubmenu, type SubmenuProps as SharedSubmenuProps } from '@dcl/one-ui'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'

export type SubmenuProps = Omit<SharedSubmenuProps, 'whileOpen'>

export function Submenu(props: SubmenuProps): React.JSX.Element {
  return <SharedSubmenu {...props} whileOpen={registerCancelLayer} />
}
