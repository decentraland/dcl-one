import { useEffect, useRef } from 'react'
import type { HTMLAttributes, ReactNode } from 'react'
import { CloseButton } from '../CloseButton/CloseButton'
import { GeneralCheck, GeneralWarning } from '../../foundations/icons/general'
import styles from './Toast.module.css'

export type ToastVariant = 'text' | 'confirmation' | 'error'

export type ToastProps = Omit<HTMLAttributes<HTMLDivElement>, 'role' | 'children'> & {
  /**
   * `status` (default) is announced when the reader is idle. `alert` is announced at once: use it
   * for an error the reader must act on.
   */
  role?: 'status' | 'alert'
  /** The message. With none the live region stays in the page, empty, ready for the next one. */
  children?: ReactNode
  variant?: ToastVariant
  /**
   * How long the message shows, in ms, before `onClose` is called. Defaults to 4000; `0` keeps it
   * until the caller removes it. The timer restarts for new text or a different message key.
   * The timer waits while the pointer or focus is on the toast.
   */
  duration?: number
  /** Identifies a message with element children, allowing it to restart the duration. */
  messageKey?: string | number
  /** Called when the time ends or the close button is pressed. The caller then removes the message. */
  onClose?: () => void
  /** Shows the close button. Needs `onClose`. */
  closeButton?: boolean
  closeLabel?: string
  action?: ReactNode
  /** `viewport` (default) fixes the toast at the bottom of the window; `inline` leaves it in the flow. */
  placement?: 'viewport' | 'inline'
  /** A leading visual, such as a profile picture. It replaces the variant's mark. */
  icon?: ReactNode
  /**
   * Not in Figma. `hud` is the Explorer HUD's notice: a borderless dark card with 14px text and no mark, where
   * `error` adds a ruby outline and only a toast with an action or a close button takes the pointer.
   */
  appearance?: 'default' | 'hud'
}

/**
 * A short message about the result of an action. The root is a live region (`status`, or `alert`)
 * that stays mounted, so assistive technology announces each message placed in it. The caller owns the
 * message: it renders the text, and removes it when `onClose` is called.
 */
export function Toast({
  children,
  role = 'status',
  variant = 'text',
  duration = 4000,
  messageKey,
  onClose,
  closeButton = false,
  closeLabel = 'Close',
  action,
  placement = 'viewport',
  icon,
  appearance = 'default',
  className,
  ...rest
}: ToastProps) {
  const shown = children !== undefined && children !== null && children !== false && children !== ''
  const timerKey = messageKey ?? (typeof children === 'string' ? children : undefined)
  const close = useRef(onClose)
  const held = useRef(false)
  useEffect(() => {
    close.current = onClose
  })
  useEffect(() => {
    if (!shown || duration <= 0) return
    let timer: ReturnType<typeof setTimeout>
    // While the reader points at or focuses the toast, the time waits and then starts again.
    const tick = () => {
      if (held.current) timer = setTimeout(tick, 250)
      else close.current?.()
    }
    timer = setTimeout(tick, duration)
    return () => clearTimeout(timer)
  }, [shown, duration, timerKey])
  const hold = (value: boolean) => () => {
    held.current = value
  }
  return (
    <div
      {...rest}
      role={role}
      className={[styles.region, className].filter(Boolean).join(' ')}
      data-placement={placement}
    >
      {shown && (
        <div
          className={styles.toast}
          data-variant={variant}
          data-appearance={appearance === 'hud' ? 'hud' : undefined}
          data-rich={closeButton || action ? true : undefined}
          onPointerEnter={hold(true)}
          onPointerLeave={hold(false)}
          onFocus={hold(true)}
          onBlur={hold(false)}
        >
          {icon}
          {icon == null && appearance !== 'hud' && variant === 'confirmation' && (
            <GeneralCheck className={styles.icon} size={24} aria-hidden="true" focusable="false" />
          )}
          {icon == null && appearance !== 'hud' && variant === 'error' && (
            <GeneralWarning className={styles.icon} size={24} aria-hidden="true" focusable="false" />
          )}
          <span className={styles.message}>{children}</span>
          {action}
          {closeButton && onClose && (
            <CloseButton variant="plain" aria-label={closeLabel} onClick={onClose} />
          )}
        </div>
      )}
    </div>
  )
}
