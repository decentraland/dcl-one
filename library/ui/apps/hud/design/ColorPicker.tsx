// ColorPicker — the shared ColorPicker. While its popup is open it is a cancel layer, so the engine's Cancel
// action (Escape or gamepad) closes just the popup, not the Backpack or the passport around it.

import { ColorPicker as SharedColorPicker, type ColorPickerProps } from '@dcl/one-ui'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'

export function ColorPicker(props: Omit<ColorPickerProps, 'whileOpen'>): React.JSX.Element {
  return <SharedColorPicker {...props} whileOpen={registerCancelLayer} />
}
