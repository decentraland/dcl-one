// Button — the shared pill CTA (primary / secondary / ghost · sm / md / lg).
// Rebuilt in TS + CSS Modules; design references eordano/dcl-react-ui Button.

import styles from './Button.module.css'

type Variant = 'primary' | 'secondary' | 'ghost' | 'light' | 'outline' | 'accent' | 'dark' | 'tint' | 'scrim' | 'faint' | 'veil' | 'glass' | 'shade' | 'clear'
type Size = 'sm' | 'md' | 'lg' | 'row' | 'card' | 'cardSm' | 'header' | 'modal' | 'cta' | 'bare'
/** Overrides the size's corner radius. */
type Radius = 'md' | 'button' | 'pill'

interface Common {
  variant?: Variant
  size?: Size
  radius?: Radius
}
// Renders a <button>, or — when `href` is given — an <a> styled identically, so links that need the
// pill CTA reuse the primitive instead of re-rolling its CSS.
type ButtonProps =
  | (Common & React.ButtonHTMLAttributes<HTMLButtonElement> & { href?: undefined })
  | (Common & React.AnchorHTMLAttributes<HTMLAnchorElement> & { href: string })

const RADIUS = { md: 'radiusMd', button: 'radiusButton', pill: 'radiusPill' } as const

export function Button(props: ButtonProps): React.JSX.Element {
  const { variant = 'primary', size = 'md', radius, className = '', ...rest } = props
  const cls = `${styles.btn} ${styles[variant]} ${styles[size]} ${radius != null ? styles[RADIUS[radius]] : ''} ${className}`.replace(/\s+/g, ' ').trim()
  if (rest.href != null) {
    return <a className={cls} {...(rest as React.AnchorHTMLAttributes<HTMLAnchorElement>)} />
  }
  const { type = 'button', ...btnRest } = rest as React.ButtonHTMLAttributes<HTMLButtonElement>
  return <button type={type} className={cls} {...btnRest} />
}
