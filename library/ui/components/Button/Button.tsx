import type {
  AnchorHTMLAttributes,
  ButtonHTMLAttributes,
  KeyboardEvent,
  MouseEvent,
  ReactNode,
  Ref
} from 'react'
import styles from './Button.module.css'
import { secureRel } from '../../link.helpers'

type AccessibleName = { 'aria-label': string } | { 'aria-labelledby': string }
type NamedContent = Exclude<ReactNode, null | undefined | boolean>
type Content = { children: NamedContent } | { label: NamedContent } | AccessibleName
type Variant =
  | 'primary'
  | 'secondary'
  | 'secondaryLight'
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

/** The Explorer HUD's looks: a fill or an outline over a scene, a panel or a dialog. */
export type ButtonHudVariant =
  | 'primary'
  | 'secondary'
  | 'ghost'
  | 'light'
  | 'outline'
  | 'accent'
  | 'dark'
  | 'tint'
  | 'scrim'
  | 'faint'
  | 'veil'
  | 'glass'
  | 'shade'
  | 'clear'
  | 'darkOutline'
  | 'whisper'
  | 'method'
  | 'navigate'
  | 'navigateExit'
/** `bare` takes its size from the caller's class and is not dimmed while disabled. */
export type ButtonHudSize =
  | 'sm'
  | 'md'
  | 'lg'
  | 'row'
  | 'card'
  | 'cardSm'
  | 'header'
  | 'modal'
  | 'dialog'
  | 'cta'
  | 'start'
  | 'bare'

type DefaultLook = {
  appearance?: 'default'
  size?: 'xl' | 'lg' | 'md' | 'sm'
  /** Responsive uses the mobile dimensions below 768px or with a coarse primary pointer. */
  viewport?: 'desktop' | 'mobile' | 'responsive'
  loading?: boolean
  leadingIcon?: ReactNode
  trailingIcon?: ReactNode
  startIcon?: ReactNode
  endIcon?: ReactNode
  forceState?: 'hover' | 'pressed' | 'focus'
  contentLayout?: 'intrinsic' | 'fill'
} & ({ iconOnly?: false } | ({ iconOnly: true } & AccessibleName)) &
  ({ variant?: Exclude<Variant, 'iconOnly'> } | ({ variant: 'iconOnly' } & AccessibleName)) &
  ({ tone?: never } | { tone?: 'danger'; variant?: 'primary' | 'outline'; iconOnly?: false }) &
  (
    | { round?: never }
    | {
        /**
         * A small round icon-only button this many pixels across, set beside text or on a picture.
         * It takes no `size` or `viewport`, and three looks: `text` (no fill, the secondary text
         * colour), `outline` (the surface fill with a hairline border) and `dark` (the brand surface
         * fill with a faint 2px ring, for a picture on a dialog). Figma's `button-icon` set draws
         * round icons at 16, 22, 32, 40, 48 and 56px without a border, so these sizes and looks
         * follow the Sites controls instead.
         */
        round: ButtonRound
        iconOnly: true
        variant: 'text' | 'outline' | 'dark'
        size?: never
        viewport?: never
        tone?: never
      }
  ) &
  (
    | { iconSize?: never }
    | {
        /** Not in Figma: the width and height of an icon-only Button's icon, whatever the icon's own size. */
        iconSize: ButtonIconSize
        iconOnly: true
      }
    | { iconSize: ButtonIconSize; variant: 'iconOnly' }
  )

/** The diameters of a `round` icon-only Button. */
export type ButtonRound = 24 | 28 | 34
/** The icon sizes an icon-only Button can force. */
export type ButtonIconSize = 14 | 16 | 20 | 24

/** The fill of the Explorer HUD's icon control. */
export type ButtonHudControlVariant = 'ghost' | 'solid' | 'dark' | 'faint'

/**
 * Not in Figma: the Explorer HUD's button. Its content is the button's own children, laid out in a
 * row 8px apart, and it has no loading state or icon slots.
 */
type HudLook = {
  appearance: 'hud'
  control?: never
  active?: never
  variant?: ButtonHudVariant
  size?: ButtonHudSize
  /** Overrides the size's corner radius. */
  radius?: 'md' | 'button' | 'pill'
}

/**
 * Not in Figma: the Explorer HUD's icon control (close, back, a menu's toggle, a count). `square`,
 * `circle` and `pill` are a 30px box (`sm` 26px, `lg` 32px; a pill grows to its content) with a
 * `ghost` fill unless `variant` says otherwise. As a button it sets `aria-pressed` from `active`;
 * pass `aria-pressed={undefined}` for a control that is not a toggle. As a link it sets none.
 */
type HudControlLook = {
  appearance: 'hud'
  control: 'square' | 'circle' | 'pill'
  variant?: ButtonHudControlVariant
  size?: 'sm' | 'md' | 'lg'
  /** The selected or pressed look. */
  active?: boolean
  radius?: never
}

/**
 * Not in Figma: the dark button of a full-screen HUD header, with a faint ring that keeps it
 * readable over a busy background. `header` is a 40px circle and `headerPill` a 46px pill.
 */
type HudHeaderLook = {
  appearance: 'hud'
  control: 'header' | 'headerPill'
  variant?: never
  size?: never
  active?: never
  radius?: never
}

export type ButtonProps = NativeProps &
  Content & {
    /**
     * Button owns its content, so native HTML cannot replace it. Omit this key before spreading a
     * typed `ButtonHTMLAttributes` or `AnchorHTMLAttributes` object into Button.
     */
    dangerouslySetInnerHTML?: never
    children?: ReactNode
    label?: ReactNode
  } & (DefaultLook | HudLook | HudControlLook | HudHeaderLook)

type HudProps<T = ButtonProps> = T extends { appearance: 'hud' } ? Omit<T, 'appearance'> : never
type DefaultProps<T = ButtonProps> = T extends { appearance: 'hud' } ? never : Omit<T, 'appearance'>

function hudLook(
  control: HudProps['control'],
  variant: HudProps['variant'],
  size: HudProps['size'],
  radius: HudProps['radius'],
  active: boolean
) {
  if (control === undefined)
    return {
      className: styles.hud,
      'data-variant': variant ?? 'primary',
      'data-size': size ?? 'md',
      'data-radius': radius
    }
  if (control === 'header' || control === 'headerPill')
    return { className: styles.hudHeader, 'data-shape': control === 'header' ? 'circle' : 'pill' }
  return {
    className: styles.hudControl,
    'data-shape': control,
    'data-variant': variant ?? 'ghost',
    'data-size': size ?? 'md',
    'data-active': active || undefined
  }
}

function HudButton({
  variant,
  size,
  radius,
  control,
  active = false,
  className,
  children,
  label,
  disabled = false,
  ...native
}: HudProps) {
  const look = hudLook(control, variant, size, radius, active)
  const common = { ...look, className: [look.className, className].filter(Boolean).join(' ') }
  const toggle = control === 'square' || control === 'circle' || control === 'pill'
  const content = children === undefined ? label : children
  if (typeof native.href === 'string') {
    const { href, target, rel, onClick, ...anchor } = native
    return (
      <a
        {...anchor}
        {...common}
        dangerouslySetInnerHTML={undefined}
        href={disabled ? undefined : href}
        target={target}
        rel={secureRel(target, rel)}
        role={disabled ? (anchor.role ?? 'link') : anchor.role}
        aria-disabled={disabled || anchor['aria-disabled']}
        data-disabled={disabled || undefined}
        onClick={
          disabled ? (event: MouseEvent<HTMLAnchorElement>) => event.preventDefault() : onClick
        }
      >
        {content}
      </a>
    )
  }
  const { type = 'button', ...button } = native as ButtonHTMLAttributes<HTMLButtonElement> & {
    ref?: Ref<HTMLButtonElement>
  }
  return (
    <button
      aria-pressed={toggle ? active : undefined}
      {...button}
      {...common}
      dangerouslySetInnerHTML={undefined}
      type={type}
      disabled={disabled}
    >
      {content}
    </button>
  )
}

export function Button({ appearance = 'default', ...props }: ButtonProps) {
  if (appearance === 'hud') return <HudButton {...(props as HudProps)} />
  return <DefaultButton {...(props as DefaultProps)} />
}

function DefaultButton(props: DefaultProps) {
  const {
    children,
    label,
    variant = 'primary',
    size = 'xl',
    tone,
    viewport = 'desktop',
    loading = false,
    leadingIcon,
    trailingIcon,
    startIcon,
    endIcon,
    iconOnly = false,
    round,
    iconSize,
    forceState,
    className,
    contentLayout = 'intrinsic',
    disabled = false,
    ...native
  } = props
  const layout = contentLayout === 'fill' ? 'fill' : 'intrinsic'
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
      <span
        className={styles.content}
        data-loading={loading || undefined}
        data-content-layout={compact ? 'intrinsic' : layout}
      >
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
    'data-tone': tone,
    'data-size': size,
    'data-viewport': viewport,
    'data-icon-only': compact || undefined,
    'data-round': (compact && round) || undefined,
    'data-icon-size': (compact && iconSize) || undefined,
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
        dangerouslySetInnerHTML={undefined}
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
      dangerouslySetInnerHTML={undefined}
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
