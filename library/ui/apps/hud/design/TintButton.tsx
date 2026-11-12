// TintButton — a button that is a translucent fill over what is behind it, with an optional
// label and icon. The tone gives the fill at rest, under the pointer and pressed; size comes from
// the caller's class.
import styles from './TintButton.module.css'

type Tone = 'dark' | 'faint' | 'glass' | 'brand' | 'clear'

interface TintButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  tone?: Tone
  /** Corner radius in px; `pill` rounds the ends fully. */
  radius?: number | 'pill'
}

export function TintButton({ tone = 'dark', radius = 12, className = '', type = 'button', style, children, ...rest }: TintButtonProps): React.JSX.Element {
  return (
    <button type={type} className={`${styles.btn} ${styles[tone]} ${className}`.trim()} style={{ borderRadius: radius === 'pill' ? 'var(--r-pill)' : radius, ...style }} {...rest}>
      {children}
    </button>
  )
}
