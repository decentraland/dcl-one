import type { ButtonHTMLAttributes } from 'react'
import { GeneralClose, GeneralCloseThin } from '../../foundations/icons/general'
import styles from './CloseButton.module.css'

/**
 * `solid`: the 46px black button of the Explorer bar. `filled`: the 40px dark button that closes a
 * section or a whole view. `plain`: the 32px button without a background, for modals, panels and toasts.
 */
export type CloseButtonVariant = 'solid' | 'filled' | 'plain'

export type CloseButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> & {
  variant?: CloseButtonVariant
  /** The button has no visible text. Its name defaults to "Close"; pass the localized word. */
  'aria-label'?: string
}

/** The thick cross of the Figma `CloseCTA`; the other variants use the foundations' close icons. */
function SolidCross() {
  return (
    <svg className={styles.icon} width={17} height={17} viewBox="0 0 17 17" fill="none" focusable="false" aria-hidden="true">
      <path
        fill="currentColor"
        fillRule="evenodd"
        clipRule="evenodd"
        d="M16.448 0.553C17.185 1.291 17.184 2.486 16.447 3.223L3.222 16.435C2.484 17.172 1.289 17.172 0.552 16.434C-0.185 15.697 -0.184 14.502 0.553 13.765L13.778 0.552C14.516 -0.185 15.711 -0.184 16.448 0.553Z"
      />
      <path
        fill="currentColor"
        fillRule="evenodd"
        clipRule="evenodd"
        d="M16.448 16.435C15.711 17.172 14.516 17.173 13.778 16.436L0.553 3.223C-0.184 2.487 -0.185 1.291 0.552 0.554C1.289 -0.184 2.484 -0.184 3.222 0.553L16.447 13.765C17.184 14.502 17.185 15.697 16.448 16.435Z"
      />
    </svg>
  )
}

/**
 * An icon-only button that closes what holds it. It is a native button: Enter and Space press it,
 * and a disabled one is really disabled. The caller decides what closing means in `onClick`.
 */
export function CloseButton({
  variant = 'solid',
  className,
  type = 'button',
  'aria-label': ariaLabel = 'Close',
  ...rest
}: CloseButtonProps) {
  return (
    <button
      {...rest}
      type={type}
      aria-label={ariaLabel}
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-variant={variant}
    >
      {variant === 'solid' ? (
        <SolidCross />
      ) : variant === 'filled' ? (
        <GeneralClose className={styles.icon} size={32} focusable="false" aria-hidden="true" />
      ) : (
        <GeneralCloseThin className={styles.icon} size={24} focusable="false" aria-hidden="true" />
      )}
    </button>
  )
}
