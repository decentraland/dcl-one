// TintButton — a button that is a translucent fill over what is behind it. The tone gives the fill
// at rest, under the pointer and pressed; size comes from the caller's class.
import type { CSSProperties } from 'react'
import styles from './TintButton.module.css'

type Tone = 'dark' | 'faint' | 'veil' | 'glass' | 'shade' | 'brand' | 'clear'

interface TintButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  tone?: Tone
  /** In px; `pill` rounds the ends fully. */
  radius?: number | 'pill'
}

export function TintButton({ tone = 'dark', radius = 12, className = '', type = 'button', style, children, ...rest }: TintButtonProps): React.JSX.Element {
  const vars = { '--tint-radius': radius === 'pill' ? 'var(--r-pill)' : `${radius}px` } as CSSProperties
  return (
    <button type={type} className={`${styles.btn} ${styles[tone]} ${className}`.trim()} style={{ ...vars, ...style }} {...rest}>
      {children}
    </button>
  )
}
