// TintButton — a button that is a translucent fill over what is behind it. The tone gives the fill
// at rest, under the pointer and pressed; size comes from the caller's class.
import type { CSSProperties } from 'react'
import styles from './TintButton.module.css'

type Tone = 'dark' | 'faint' | 'veil' | 'glass' | 'shade' | 'brand' | 'clear'

interface TintButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  tone?: Tone
  /** The corner token: `--r-md`, `--r-button` or `--r-pill`. */
  radius?: 'md' | 'button' | 'pill'
}

export function TintButton({ tone = 'dark', radius = 'md', className = '', type = 'button', style, children, ...rest }: TintButtonProps): React.JSX.Element {
  const vars = { '--tint-radius': `var(--r-${radius})` } as CSSProperties
  return (
    <button type={type} className={`${styles.btn} ${styles[tone]} ${className}`.trim()} style={{ ...vars, ...style }} {...rest}>
      {children}
    </button>
  )
}
