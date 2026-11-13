// Spinner — circular loading indicator (ruby arc on a faint track).
// Design references eordano/dcl-react-ui.

import styles from './Spinner.module.css'

interface SpinnerProps {
  size?: number
  color?: string
  /** `shop`: the Shop's — a thick 40%-black ring with a quarter arc, stroked on the box's edge.
   *  `chase`: the sign-in screens' — no track; an arc grows to a circle, its tail chases its head, and
   *  it turns orange, ruby, purple, a second each. */
  variant?: 'default' | 'shop' | 'chase'
}

export function Spinner({ size = 28, color, variant = 'default' }: SpinnerProps): React.JSX.Element {
  const style = {
    '--sz': `${size}px`,
    ...(color ? { '--spinner-arc': color } : {})
  } as React.CSSProperties
  if (variant === 'shop') {
    // The design's 64px ring: radius 32 on the box's edge, a 8.53 stroke spilling 4.27 past it.
    const bleed = (4.2667 / 64) * size
    return (
      <span className={`${styles.spinner} ${styles.shop}`} style={style} role="status" aria-label="Loading">
        <svg viewBox="-4.2667 -4.2667 72.5333 72.5333" width={size + 2 * bleed} height={size + 2 * bleed} style={{ margin: -bleed }}>
          <circle className={styles.track} cx="32" cy="32" r="32" fill="none" strokeWidth="8.5333" />
          <path className={styles.arc} d="M32 0A32 32 0 0 1 64 32" fill="none" strokeWidth="8.5333" strokeLinecap="round" />
        </svg>
      </span>
    )
  }
  if (variant === 'chase') {
    return (
      <span className={`${styles.spinner} ${styles.chase}`} style={style} role="status" aria-label="Loading">
        <svg viewBox="0 0 50 50" width={size} height={size}>
          <circle className={styles.arc} cx="25" cy="25" r="20" fill="none" strokeWidth="4" strokeLinecap="round" pathLength="100" />
        </svg>
      </span>
    )
  }
  return (
    <span className={styles.spinner} style={style} role="status" aria-label="Loading">
      <svg viewBox="0 0 50 50" width={size} height={size}>
        <circle className={styles.track} cx="25" cy="25" r="20" fill="none" strokeWidth="5" />
        <circle
          className={styles.arc}
          cx="25"
          cy="25"
          r="20"
          fill="none"
          strokeWidth="5"
          strokeLinecap="round"
          strokeDasharray="90 160"
        />
      </svg>
    </span>
  )
}
