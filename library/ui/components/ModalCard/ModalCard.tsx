// The Explorer HUD's dialog card and its parts: ModalCard (the card), ModalTitle (its header), ModalActions (its
// footer), ModalShell (the three laid out) and ModalBackdrop (the layer that centres a card over the screen). Modal
// is the dialog itself, with its own backdrop and focus.

import { Children, Fragment, isValidElement, type HTMLAttributes, type ReactNode, type Ref } from 'react'
import styles from './ModalCard.module.css'

export interface ModalCardProps {
  children: ReactNode
  width?: number
  ariaLabel?: string
  role?: string
  /** `false` sets `aria-modal="false"`. */
  modal?: boolean
  className?: string
}

/**
 * Not in Figma: the Explorer HUD's dialog card. It is presentational: whoever hosts it owns the scrim, the scale,
 * the entrance animation, the focus trap and Escape. A click on the card does not reach the host's scrim.
 */
export function ModalCard({
  children,
  width = 420,
  ariaLabel,
  role = 'dialog',
  modal = true,
  className = ''
}: ModalCardProps) {
  return (
    <div
      className={`${styles.card} ${className}`.trim()}
      style={{ width }}
      role={role}
      aria-modal={modal}
      aria-label={ariaLabel}
      onClick={(e) => e.stopPropagation()}
    >
      {children}
    </div>
  )
}

export interface ModalTitleProps {
  title?: ReactNode
  subtitle?: ReactNode
  icon?: ReactNode
  onClose?: () => void
  onBack?: () => void
  closeLabel?: string
  backLabel?: string
  closeSize?: number
  centered?: boolean
  className?: string
}

export function ModalTitle({
  title,
  subtitle,
  icon,
  onClose,
  onBack,
  closeLabel = 'Close',
  backLabel = 'Back',
  closeSize = 14,
  centered = false,
  className = ''
}: ModalTitleProps) {
  return (
    <header
      className={`${styles.title} ${centered ? styles.titleCentered : ''} ${className}`.trim()}
    >
      {onBack ? (
        <button
          type="button"
          className={styles.back}
          aria-label={backLabel}
          onClick={onBack}
        >
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
            <path
              d="M15 5l-7 7 7 7"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              fill="none"
            />
          </svg>
        </button>
      ) : null}
      <div className={styles.titleText}>
        <div className={styles.titleHeading}>
          {icon ? (
            <span className={styles.icon} aria-hidden="true">
              {icon}
            </span>
          ) : null}
          {title}
        </div>
        {subtitle ? <div className={styles.subtitle}>{subtitle}</div> : null}
      </div>
      {onClose ? (
        <button
          type="button"
          className={styles.close}
          aria-label={closeLabel}
          onClick={onClose}
        >
          <svg
            viewBox="0 0 24 24"
            width={closeSize}
            height={closeSize}
            aria-hidden="true"
          >
            <path
              d="M6 6l12 12M18 6L6 18"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
            />
          </svg>
        </button>
      ) : null}
    </header>
  )
}

type ActionsDirection = 'row' | 'column'
type ActionsAlign = 'stretch' | 'start' | 'center' | 'end' | 'between'

export interface ModalActionsProps {
  children: ReactNode
  lead?: ReactNode
  equal?: boolean
  direction?: ActionsDirection
  align?: ActionsAlign
  className?: string
}

// Flatten children, unwrapping fragments, so `equal` mode produces one column
// per button even when actions are passed as <>…</> (which React reports as a
// single child). Without this a fragment collapses into one column.
function flattenChildren(children: ReactNode): ReactNode[] {
  const out: ReactNode[] = []
  Children.forEach(children, (child) => {
    if (child == null || child === false) return
    if (isValidElement(child) && child.type === Fragment) {
      const props = child.props as { children?: ReactNode }
      out.push(...flattenChildren(props.children))
    } else {
      out.push(child)
    }
  })
  return out
}

export function ModalActions({
  children,
  lead,
  equal = false,
  direction = 'row',
  align = 'stretch',
  className = ''
}: ModalActionsProps) {
  const isColumn = direction === 'column'
  const justify =
    align === 'start'
      ? 'flex-start'
      : align === 'center'
        ? 'center'
        : align === 'end'
          ? 'flex-end'
          : align === 'between'
            ? 'space-between'
            : undefined

  const wrapped = equal
    ? flattenChildren(children).map((child, i) => (
        <div className={styles.actionBtn} key={i}>
          {child}
        </div>
      ))
    : children

  // With a lead note the buttons must stay grouped so they wrap together (and
  // stay right-aligned) instead of the note being crushed into a thin column.
  const hasLead = lead != null && !isColumn
  const actions = hasLead ? <div className={styles.group}>{wrapped}</div> : wrapped

  return (
    <div
      className={`${styles.actions} ${isColumn ? styles.actionsColumn : ''} ${
        hasLead ? styles.actionsLead : ''
      } ${className}`.trim()}
      style={justify ? { justifyContent: justify } : undefined}
    >
      {lead != null ? <div className={styles.lead}>{lead}</div> : null}
      {actions}
    </div>
  )
}

const SIZES: Record<string, number> = {
  sm: 420,
  tiny: 420,
  md: 540,
  small: 540,
  lg: 720,
  large: 720,
  xl: 900
}

export interface ModalShellProps {
  children: ReactNode
  /** Runs from the header close (X) button, when `closeButton` is on. */
  onClose?: () => void
  width?: number
  size?: keyof typeof SIZES
  className?: string
  ariaLabel?: string
  role?: string
  modal?: boolean
  bodyless?: boolean
  bodyClassName?: string
  header?: ReactNode
  title?: ReactNode
  subtitle?: ReactNode
  icon?: ReactNode
  onBack?: () => void
  closeButton?: boolean
  centeredTitle?: boolean
  closeSize?: number
  actions?: ReactNode
  actionsEqual?: boolean
  actionsDirection?: ActionsDirection
  actionsAlign?: ActionsAlign
  actionsLead?: ReactNode
  actionsClassName?: string
}

export function ModalShell({
  children,
  onClose,
  width,
  size = 'sm',
  className = '',
  ariaLabel,
  role = 'dialog',
  modal = true,
  bodyless = false,
  bodyClassName = '',
  header,
  title,
  subtitle,
  icon,
  onBack,
  closeButton = true,
  centeredTitle = false,
  closeSize,
  actions,
  actionsEqual = false,
  actionsDirection = 'row',
  actionsAlign = 'stretch',
  actionsLead,
  actionsClassName
}: ModalShellProps) {
  const resolvedWidth = width != null ? width : (SIZES[size] ?? SIZES.sm)

  const hasAutoHeader =
    title != null || subtitle != null || icon != null || onBack != null
  const headerNode =
    header != null ? (
      header
    ) : hasAutoHeader ? (
      <ModalTitle
        title={title}
        subtitle={subtitle}
        icon={icon}
        onBack={onBack}
        onClose={closeButton ? onClose : undefined}
        centered={centeredTitle}
        closeSize={closeSize}
      />
    ) : null

  const footerNode =
    actions != null ? (
      <ModalActions
        equal={actionsEqual}
        direction={actionsDirection}
        align={actionsAlign}
        lead={actionsLead}
        className={actionsClassName}
      >
        {actions}
      </ModalActions>
    ) : null

  const body = bodyless ? (
    children
  ) : (
    <div className={`${styles.shellBody} ${bodyClassName}`.trim()}>{children}</div>
  )

  return (
    <ModalCard width={resolvedWidth} className={`${styles.shell} ${className}`.trim()} ariaLabel={ariaLabel} role={role} modal={modal}>
      {headerNode}
      {body}
      {footerNode}
    </ModalCard>
  )
}

export interface ModalBackdropProps extends Omit<HTMLAttributes<HTMLDivElement>, 'children'> {
  children?: ReactNode
  /**
   * `dim` (default) is the blurred purple scrim with the content centred on it; `deep` is darker and not blurred,
   * for a panel that stands alone over a busy page; `clear` draws nothing and only covers the screen, for content
   * the host places itself.
   */
  tone?: 'dim' | 'deep' | 'clear'
  /** Draws a `dim` or `deep` scrim at device pixels, and its content at a scale of 1, whatever the host's scale. */
  fixed?: boolean
  ref?: Ref<HTMLDivElement>
}

/**
 * Not in Figma: the layer the Explorer HUD's dialog cards open on. It covers the screen at the host's `--ui-scale`
 * (1 without one), so content sized in reference pixels is centred and scaled with the rest of the HUD. It is
 * presentational: its host owns the stack, the focus trap, Escape and what a click on it does.
 */
export function ModalBackdrop({ children, tone = 'dim', fixed = false, className = '', ref, ...rest }: ModalBackdropProps) {
  const scrim = tone !== 'clear'
  return (
    <div
      {...rest}
      ref={ref}
      className={`${styles.backdrop} ${className}`.trim()}
      data-tone={scrim ? tone : undefined}
      data-fixed={scrim && fixed ? true : undefined}
    >
      {scrim ? <div className={styles.pop}>{children}</div> : children}
    </div>
  )
}
