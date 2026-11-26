// Select — the shared Dropdown in its HUD look: a field and a popup list, used by Settings (resolution,
// quality, …) and anywhere a choice is needed.

import { Dropdown } from '@dcl/one-ui'

export interface SelectOption {
  value: string
  label: string
}

interface SelectProps {
  value: string
  options: SelectOption[]
  onChange: (value: string) => void
  disabled?: boolean
  /** dark (default, on dark panels) or light (white field, e.g. Settings). */
  variant?: 'dark' | 'light'
  'aria-label'?: string
  /** Ids of elements that describe the control. */
  'aria-describedby'?: string
  /** Shown in the field while no option has the value (a picker that adds rather than selects). */
  placeholder?: string
}

export function Select({
  value,
  options,
  onChange,
  disabled = false,
  variant = 'dark',
  'aria-label': ariaLabel,
  'aria-describedby': ariaDescribedBy,
  placeholder
}: SelectProps): React.JSX.Element {
  return (
    <Dropdown
      appearance="hud"
      value={value}
      options={options}
      onChange={onChange}
      disabled={disabled}
      variant={variant}
      label={ariaLabel}
      aria-describedby={ariaDescribedBy}
      placeholder={placeholder}
    />
  )
}
