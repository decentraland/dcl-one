import { forwardRef } from 'react'
import type {
  ComponentPropsWithoutRef,
  ElementType,
  ForwardedRef,
  MouseEventHandler,
  ReactElement,
  ReactNode
} from 'react'
import { cx } from '~/lib/cx'
import styles from './Button.module.css'

export type ButtonVariant = 'primary' | 'gradient' | 'secondary' | 'dark' | 'glass' | 'light' | 'ghost'
export type ButtonSize = 'md' | 'sm' | 'icon' | 'compact' | 'lg' | 'hero'

type ButtonOwnProps = {
  variant?: ButtonVariant
  size?: ButtonSize
  /** Replaces the content with a spinner and disables the button. */
  loading?: boolean
  disabled?: boolean
  children?: ReactNode
  /** Merged after the button's own class: layout tweaks from a consumer's module win over the foundation layer. */
  className?: string
  // Pinned explicitly so it stays typed when the polymorphic element type widens to ElementType.
  onClick?: MouseEventHandler<HTMLElement>
}

// Polymorphic: a <button> by default; `as="a"` (or a router Link) keeps the styling on a link.
type ButtonProps<C extends ElementType> = ButtonOwnProps & { as?: C } & Omit<
    ComponentPropsWithoutRef<C>,
    keyof ButtonOwnProps | 'as'
  >

function ButtonInner<C extends ElementType = 'button'>(
  { as, variant = 'primary', size = 'md', loading = false, disabled, className, children, ...rest }: ButtonProps<C>,
  ref: ForwardedRef<Element>
) {
  // Polymorphic `as` + ref can't be statically reconciled, so the strict typing lives on the exported signature.
  const Root: ElementType = as ?? 'button'
  return (
    <Root
      ref={ref}
      className={cx(styles.root, className)}
      data-variant={variant}
      data-size={size}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading ? <span className={styles.spinner} data-testid="button-spinner" aria-hidden /> : children}
    </Root>
  )
}

// forwardRef erases the generic, so re-assert the polymorphic call signature.
export const Button = forwardRef(ButtonInner) as <C extends ElementType = 'button'>(
  props: ButtonProps<C> & { ref?: ForwardedRef<Element> }
) => ReactElement
