// Toggle (switch) — DCL ruby pill with a white knob (matches the Unity settings
// "Fullscreen" control). Used by Settings and anywhere a boolean control is needed.

import styles from './Toggle.module.css'

interface ToggleProps {
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
  /** sm: the compact 39×24 switch used in HUD popovers. */
  size?: 'md' | 'sm'
  /** light: the switch sits on a light panel, so its off track is dark. */
  surface?: 'dark' | 'light'
  'aria-label'?: string
}

export function Toggle({
  checked,
  onChange,
  disabled = false,
  size = 'md',
  surface = 'dark',
  'aria-label': ariaLabel
}: ToggleProps): React.JSX.Element {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      disabled={disabled}
      className={`${styles.track} ${size === 'sm' ? styles.sm : ''} ${surface === 'light' ? styles.onLight : ''} ${checked ? styles.on : ''}`.trim()}
      onClick={() => onChange(!checked)}
    >
      <span className={styles.knob} />
    </button>
  )
}
