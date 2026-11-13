import { secureRel } from '../../link.helpers'
import type {
  AnchorHTMLAttributes,
  ButtonHTMLAttributes,
  KeyboardEvent,
  MouseEvent,
  ReactNode,
  Ref
} from 'react'
import styles from './Button.module.css'

type AccessibleName = { 'aria-label': string } | { 'aria-labelledby': string }
type NamedContent = Exclude<ReactNode, null | undefined | boolean>
type Content = { children: NamedContent } | { label: NamedContent } | AccessibleName
type Variant =
  | 'primary'
  | 'secondary'
  | 'outline'
  | 'dark'
  | 'text'
  | 'outlineOnLight'
  | 'tertiary'
  | 'quaternary'
  | 'inverse'
  | 'iconOnly'
type NativeProps =
  | (Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> & {
      href?: never
      ref?: Ref<HTMLButtonElement>
    })
  | (Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'children' | 'type'> & {
      href: string
      disabled?: boolean
      type?: never
      ref?: Ref<HTMLAnchorElement>
    })

export type ButtonProps = NativeProps &
  Content & {
    children?: ReactNode
    label?: ReactNode
    size?: 'xl' | 'lg' | 'md' | 'sm'
    viewport?: 'desktop' | 'mobile'
    loading?: boolean
    leadingIcon?: ReactNode
    trailingIcon?: ReactNode
    startIcon?: ReactNode
    endIcon?: ReactNode
    forceState?: 'hover' | 'pressed' | 'focus'
  } & ({ iconOnly?: false } | ({ iconOnly: true } & AccessibleName)) &
  ({ variant?: Exclude<Variant, 'iconOnly'> } | ({ variant: 'iconOnly' } & AccessibleName))

export function Button({
  children,
  label,
  variant = 'primary',
  size = 'xl',
  viewport = 'desktop',
  loading = false,
  leadingIcon,
  trailingIcon,
  startIcon,
  endIcon,
  iconOnly = false,
  forceState,
  className,
  disabled = false,
  ...native
}: ButtonProps) {
  const compact = iconOnly || variant === 'iconOnly'
  const unavailable =
    disabled || loading || native['aria-disabled'] === true || native['aria-disabled'] === 'true'
  const classes = [styles.root, className].filter(Boolean).join(' ')
  const appearance =
    variant === 'tertiary'
      ? 'outline'
      : variant === 'quaternary'
        ? 'dark'
        : variant === 'iconOnly'
          ? 'primary'
          : variant
  const content = (
    <>
      <span className={styles.content} data-loading={loading || undefined}>
        {(leadingIcon ?? startIcon) != null && (
          <span className={styles.icon} aria-hidden="true">
            {leadingIcon ?? startIcon}
          </span>
        )}
        <span className={styles.label} data-button-label>
          {children === undefined ? label : children}
        </span>
        {(trailingIcon ?? endIcon) != null && (
          <span className={styles.icon} aria-hidden="true">
            {trailingIcon ?? endIcon}
          </span>
        )}
      </span>
      {loading && <span className={styles.spinner} data-loading-indicator aria-hidden="true" />}
    </>
  )
  const common = {
    className: classes,
    'data-variant': appearance,
    'data-size': size,
    'data-viewport': viewport,
    'data-icon-only': compact || undefined,
    'data-force-state': forceState,
    'aria-busy': loading || native['aria-busy'],
    'aria-disabled': unavailable || native['aria-disabled']
  }
  const guardClick =
    <T extends HTMLElement>(handler: ((event: MouseEvent<T>) => void) | undefined) =>
    (event: MouseEvent<T>) => {
      if (unavailable) {
        event.preventDefault()
        return
      }
      handler?.(event)
    }
  const guardKey =
    <T extends HTMLElement>(handler: ((event: KeyboardEvent<T>) => void) | undefined) =>
    (event: KeyboardEvent<T>) => {
      if (unavailable && (event.key === 'Enter' || event.key === ' ')) {
        event.preventDefault()
        return
      }
      handler?.(event)
    }
  if (typeof native.href === 'string') {
    const { href, target, rel, onClick, onAuxClick, onKeyDown, ...anchor } = native
    return (
      <a
        {...anchor}
        {...common}
        href={unavailable ? undefined : href}
        target={target}
        rel={secureRel(target, rel)}
        role={unavailable ? (anchor.role ?? 'link') : anchor.role}
        tabIndex={disabled ? -1 : unavailable ? (anchor.tabIndex ?? 0) : anchor.tabIndex}
        onClick={guardClick(onClick)}
        onAuxClick={guardClick(onAuxClick)}
        onKeyDown={guardKey(onKeyDown)}
      >
        {content}
      </a>
    )
  }
  const {
    type = 'button',
    onClick,
    onAuxClick,
    onKeyDown,
    ...button
  } = native as ButtonHTMLAttributes<HTMLButtonElement> & { ref?: Ref<HTMLButtonElement> }
  return (
    <button
      {...button}
      {...common}
      type={type}
      disabled={disabled}
      onClick={guardClick(onClick)}
      onAuxClick={guardClick(onAuxClick)}
      onKeyDown={guardKey(onKeyDown)}
    >
      {content}
    </button>
  )
}
