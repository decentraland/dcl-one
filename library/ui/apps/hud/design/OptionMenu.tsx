// OptionMenu — the shared OptionMenu, with the HUD's check icon. While open it is a cancel layer, so the
// engine's Cancel action closes it.

import { MaskIcon, OptionMenu as SharedOptionMenu, type OptionMenuProps, type OptionSection } from '@dcl/one-ui'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import checkIcon from '../assets/backpack/icon-check.webp'

export type { OptionSection }

const check = <MaskIcon src={checkIcon} size={14} />

export function OptionMenu(props: Omit<OptionMenuProps, 'check' | 'whileOpen'>): React.JSX.Element {
  return <SharedOptionMenu {...props} check={check} whileOpen={registerCancelLayer} />
}
