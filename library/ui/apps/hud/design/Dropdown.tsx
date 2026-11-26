// Dropdown — the shared Dropdown as the HUD's sort list: plain text options with full keyboard nav (arrows,
// Home/End, Enter), controlled or uncontrolled. While its list is open it is a cancel layer, so the engine's
// Cancel action (key or gamepad) closes just the list and not the panel around it.

import { Dropdown as SharedDropdown } from '@dcl/one-ui'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'

export interface DropdownProps {
  options: string[]
  value?: string
  defaultValue?: string
  onChange?: (value: string) => void
}

export function Dropdown({ options, value, defaultValue, onChange }: DropdownProps): React.JSX.Element {
  return (
    <SharedDropdown
      appearance="hud"
      kind="sort"
      options={options}
      value={value}
      defaultValue={defaultValue}
      onChange={onChange}
      whileOpen={registerCancelLayer}
    />
  )
}
