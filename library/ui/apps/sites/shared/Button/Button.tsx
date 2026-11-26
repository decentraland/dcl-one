import type { AnchorHTMLAttributes, ButtonHTMLAttributes } from 'react'
import styles from './Button.module.css'

type ButtonVariant =
  'primary' | 'secondary' | 'tertiary' | 'quaternary' | 'inverse' | 'text' | 'iconOnly'

type ButtonSize = 'md' | 'sm'

/** An icon-only control has no visible text, so its type requires the accessible name. */
type VariantProps = { size?: ButtonSize } & (
  { variant?: Exclude<ButtonVariant, 'iconOnly'> } | { variant: 'iconOnly'; 'aria-label': string }
)

/** Without `href`: a native button, `type="button"` unless given. */
type AsButton = ButtonHTMLAttributes<HTMLButtonElement> & { href?: never }

/** With `href`: a link styled as a button, for navigation and downloads. It has no `type`. */
type AsLink = AnchorHTMLAttributes<HTMLAnchorElement> & {
  href: string
  type?: never
  disabled?: never
}

type ButtonProps = VariantProps & (AsButton | AsLink)

const SAFE_REL = ['noopener', 'noreferrer']

/** A new-tab link never gets a handle on this page, whatever rel the caller passed. */
function safeRel(rel: string | undefined): string {
  const tokens = new Set((rel ?? '').split(/\s+/).filter(Boolean))
  for (const token of SAFE_REL) tokens.add(token)
  return [...tokens].join(' ')
}

function Button(props: ButtonProps) {
  const { variant = 'primary', size = 'md', className, ...rest } = props
  const classes = [styles.root, styles[variant], size === 'sm' && styles.sm, className]
    .filter(Boolean)
    .join(' ')

  if (typeof rest.href === 'string') {
    const { rel, target, ...anchor } = rest as AnchorHTMLAttributes<HTMLAnchorElement>
    return (
      <a
        {...anchor}
        target={target}
        rel={target === '_blank' ? safeRel(rel) : rel}
        className={classes}
      />
    )
  }

  const { type = 'button', ...button } = rest as ButtonHTMLAttributes<HTMLButtonElement>
  return <button {...button} type={type} className={classes} />
}

export { Button }
export type { ButtonProps, ButtonSize, ButtonVariant }
