import styles from './Switch.module.css'

type Props = {
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
  label: string
  testId: string
}

export function Switch({ checked, onChange, disabled, label, testId }: Props) {
  return (
    <button
      className={styles.track}
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      data-checked={checked || undefined}
      data-testid={testId}
      onClick={() => onChange(!checked)}
    >
      <span className={styles.knob} aria-hidden />
    </button>
  )
}
