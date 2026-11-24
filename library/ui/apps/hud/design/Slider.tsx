// Slider — styled range input with a ruby fill + white knob (DCL settings style).

import styles from './Slider.module.css'

interface SliderProps {
  value: number
  min?: number
  max?: number
  step?: number
  onChange: (value: number) => void
  disabled?: boolean
  /** Show ‹ › stepper arrows (DCL settings style). */
  arrows?: boolean
  /** thick: a 22px track with a pin handle (HUD popovers). pin: the settings page's 30px track with a tall pin and chevron steppers.
   *  rail: a navigation popover's 12px track with a 12x26 pin; `--slider-rail-track` sets the unfilled colour. */
  variant?: 'thin' | 'thick' | 'pin' | 'rail'
  'aria-label'?: string
  /** Ids of elements that describe the control. */
  'aria-describedby'?: string
}

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
}: SliderProps): React.JSX.Element {
  const pct = max > min ? ((value - min) / (max - min)) * 100 : 0
  const clamp = (v: number): number => Math.min(max, Math.max(min, v))
  const input = (
    <input
      type="range"
      className={`${styles.slider} ${variant === 'thin' ? '' : styles[variant]}`.trim()}
      value={value}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      aria-label={ariaLabel}
      aria-describedby={ariaDescribedBy}
      style={{ '--pct': `${pct}%`, '--frac': pct / 100 } as React.CSSProperties}
      onChange={(e) => onChange(Number(e.target.value))}
    />
  )
  if (!arrows) return input
  const pin = variant === 'pin'
  const arrow = `${styles.arrow} ${pin ? styles.pinArrow : ''}`.trim()
  return (
    <div className={`${styles.row} ${pin ? styles.pinRow : ''}`.trim()}>
      <button type="button" className={arrow} disabled={disabled || value <= min} aria-label="decrease" onClick={() => onChange(clamp(value - step))}>
        ‹
      </button>
      {input}
      <button type="button" className={`${arrow} ${pin ? styles.pinNext : ''}`.trim()} disabled={disabled || value >= max} aria-label="increase" onClick={() => onChange(clamp(value + step))}>
        ›
      </button>
    </div>
  )
}
