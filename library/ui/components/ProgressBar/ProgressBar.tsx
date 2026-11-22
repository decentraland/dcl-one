import type { CSSProperties, HTMLAttributes, ReactNode } from 'react'
import styles from './ProgressBar.module.css'

type ProgressBarName =
  | { 'aria-label': string; 'aria-labelledby'?: string }
  | { 'aria-label'?: string; 'aria-labelledby': string }

export type ProgressBarProps = Omit<
  HTMLAttributes<HTMLDivElement>,
  'children' | 'role' | 'aria-valuemin' | 'aria-valuemax' | 'aria-valuenow'
> &
  ProgressBarName & {
    /** The progress made, from 0 to `max`. A value outside that range is drawn and reported at its end. */
    value: number
    max?: number
    /**
     * The drawn share of the track, from 0 to 1, for a scale that is not linear. Defaults to
     * `value / max`. The reported value stays `value`.
     */
    fraction?: number
    /** Text shown after the track, such as "1/3". It is not announced: say it in `aria-valuetext`. */
    valueLabel?: ReactNode
    /** With it, a full bar shows the check mark and this text in place of the track. */
    completedLabel?: ReactNode
  }

const clamp = (value: number, max: number) => (Number.isFinite(value) ? Math.min(Math.max(value, 0), max) : 0)

/**
 * How much of a task is done. The root is the `progressbar`, so it needs a name (`aria-label` or
 * `aria-labelledby`); `aria-valuetext` replaces the bare number for assistive technology.
 */
export function ProgressBar({
  value,
  max = 100,
  fraction,
  valueLabel,
  completedLabel,
  className,
  style,
  ...rest
}: ProgressBarProps) {
  const top = max > 0 && Number.isFinite(max) ? max : 100
  const now = clamp(value, top)
  const share = clamp(fraction ?? now / top, 1)
  const completed = completedLabel !== undefined && now >= top
  return (
    <div
      {...rest}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={top}
      aria-valuenow={now}
      className={[styles.root, className].filter(Boolean).join(' ')}
      style={{ ...style, '--progress-bar-share': `${share * 100}%` } as CSSProperties}
    >
      {completed ? (
        <span className={styles.completed}>
          <svg className={styles.check} viewBox="0 0 9 6" aria-hidden="true" focusable="false">
            <path d="M0 3.8 2.06 6 8.48 0" />
          </svg>
          {completedLabel}
        </span>
      ) : (
        <>
          <span className={styles.track}>
            <span className={styles.fill} />
          </span>
          {valueLabel !== undefined && <span className={styles.value}>{valueLabel}</span>}
        </>
      )}
    </div>
  )
}
