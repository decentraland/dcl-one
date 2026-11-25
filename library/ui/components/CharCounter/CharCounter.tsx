import type { HTMLAttributes } from 'react'
import styles from './CharCounter.module.css'

export type CharCounterProps = Omit<HTMLAttributes<HTMLDivElement>, 'children'> & {
  current?: number
  max?: number
  /** `slash` is "3/10", `spaced` "3 / 10" and `words` "(3 out of 10 characters)". */
  format?: 'slash' | 'spaced' | 'words'
  /** Turns the count red above `max`. On by default. */
  over?: boolean
}

/** The `current/max` readout under a text field. It turns red above the maximum. */
export function CharCounter({
  current = 0,
  max,
  format = 'slash',
  over = true,
  className,
  ...rest
}: CharCounterProps) {
  const text =
    format === 'spaced'
      ? `${current} / ${max}`
      : format === 'words'
        ? `(${current} out of ${max} characters)`
        : `${current}/${max}`
  const isOver = over && max != null && current > max
  return (
    <div
      {...rest}
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-over={isOver || undefined}
    >
      {text}
    </div>
  )
}
