// HintsButton — the shared HintsButton with the HUD's keyboard icon. While its list is open it is a cancel
// layer, so the engine's Cancel action (key or gamepad) closes it.

import { HintsButton as SharedHintsButton, MaskIcon, type Hint, type HintsButtonProps } from '@dcl/one-ui'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import keyboardIcon from '../assets/backpack/icon-keyboard.webp'

export type { Hint }

const keyboard = <MaskIcon src={keyboardIcon} size={22} />

export function HintsButton(props: Omit<HintsButtonProps, 'icon' | 'whileOpen'>): React.JSX.Element {
  return <SharedHintsButton {...props} icon={keyboard} whileOpen={registerCancelLayer} />
}
