import type { CSSProperties } from 'react'
import styles from './Slider.module.css'

export type SliderProps = {
  value: number
  min?: number
  max?: number
  step?: number
  onChange: (value: number) => void
  disabled?: boolean
  /** Adds a decrease and an increase button around the track; each moves the value one `step`. */
  arrows?: boolean
  /**
   * `thin` is a 6px track with a round knob. `thick` is a 22px track with a pin handle. `pin` is a
   * 30px track with a tall pin and chevron steppers. `rail` is a 12px track with a 12 by 26 pin.
   */
  variant?: 'thin' | 'thick' | 'pin' | 'rail'
  'aria-label'?: string
  /** Ids of elements that describe the control. */
  'aria-describedby'?: string
}

/** A native range input with a ruby fill up to its value. */
export function Slider({
  value,
  min = 0,
  max = 100,
  step = 1,
  onChange,
  disabled = false,
  arrows = false,
  variant = 'thin',
  'aria-label': ariaLabel,
  'aria-describedby': ariaDescribedBy
}: SliderProps) {
  const pct = max > min ? ((value - min) / (max - min)) * 100 : 0
  const clamp = (next: number) => Math.min(max, Math.max(min, next))
  const input = (
    <input
      type="range"
      className={[styles.root, variant !== 'thin' && styles[variant]].filter(Boolean).join(' ')}
      data-variant={variant}
      value={value}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      aria-label={ariaLabel}
      aria-describedby={ariaDescribedBy}
      style={{ '--pct': `${pct}%`, '--frac': pct / 100 } as CSSProperties}
      onChange={(event) => onChange(Number(event.target.value))}
    />
  )
  if (!arrows) return input
  const pin = variant === 'pin'
  const arrow = [styles.arrow, pin && styles.pinArrow].filter(Boolean).join(' ')
  return (
    <div className={[styles.row, pin && styles.pinRow].filter(Boolean).join(' ')}>
      <button
        type="button"
        className={arrow}
        disabled={disabled || value <= min}
        aria-label="decrease"
        onClick={() => onChange(clamp(value - step))}
      >
        ‹
      </button>
      {input}
      <button
        type="button"
        className={[arrow, pin && styles.pinNext].filter(Boolean).join(' ')}
        disabled={disabled || value >= max}
        aria-label="increase"
        onClick={() => onChange(clamp(value + step))}
      >
        ›
      </button>
    </div>
  )
}
