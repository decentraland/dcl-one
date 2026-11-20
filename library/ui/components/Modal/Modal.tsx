import { useEffect, useId, useRef } from 'react'
import type { DialogHTMLAttributes, ReactNode, RefObject } from 'react'
import { CheckboxLabel } from '../CheckboxLabel/CheckboxLabel'
import type { CheckboxLabelProps } from '../CheckboxLabel/CheckboxLabel'
import { CloseButton } from '../CloseButton/CloseButton'
import styles from './Modal.module.css'

export type ModalProps = Omit<
  DialogHTMLAttributes<HTMLDialogElement>,
  | 'title'
  | 'children'
  | 'open'
  | 'onClose'
  | 'onCancel'
  | 'onKeyDown'
  | 'onPointerDown'
  | 'onClick'
  | 'aria-labelledby'
> & {
  title: ReactNode
  /** Keeps the title out of view. It still names the dialog. */
  titleHidden?: boolean
  /** The body text. It is the dialog's description. */
  description?: ReactNode
  children?: ReactNode
  /** A picture or illustration above the title. A decorative one has an empty `alt`. */
  image?: ReactNode
  checkbox?: CheckboxLabelProps
  /** One or two actions, such as Buttons, the main one last. */
  actions?: ReactNode
  divider?: boolean
  /** Called when the reader closes the dialog. The caller then stops rendering it. */
  onClose: () => void
  /**
   * `false` while the dialog must stay open, for example during a request: Escape, the close button
   * and a press on the backdrop then do nothing. Defaults to `true`.
   */
  dismissible?: boolean
  /** Shows the close button in the corner. Defaults to `true`; without it an action must close the dialog. */
  closeButton?: boolean
  closeLabel?: string
  /** Where focus goes when the dialog closes. Defaults to the element that had focus when it opened. */
  returnFocusRef?: RefObject<HTMLElement | null>
  /** The element that takes focus when the dialog opens. Defaults to the first control inside. */
  initialFocusRef?: RefObject<HTMLElement | null>
}

function isOutside(dialog: HTMLDialogElement, x: number, y: number) {
  const box = dialog.getBoundingClientRect()
  return x < box.left || x > box.right || y < box.top || y > box.bottom
}

// A browser without modal dialogs still shows and hides the dialog; only the inert page is lost.
function show(dialog: HTMLDialogElement) {
  if (dialog.open) return
  if (typeof dialog.showModal === 'function') dialog.showModal()
  else dialog.setAttribute('open', '')
}

function hide(dialog: HTMLDialogElement) {
  if (!dialog.open) return
  if (typeof dialog.close === 'function') dialog.close()
  else dialog.removeAttribute('open')
}

/**
 * A modal dialog: a native `<dialog>` shown with `showModal()` while it is mounted. The browser
 * moves focus inside, keeps Tab inside and makes the page behind inert; the page does not scroll
 * behind it. Escape, the close button and a press on the backdrop call `onClose`, and focus returns
 * to where it was. The caller renders it only while it is open.
 */
export function Modal({
  title,
  titleHidden = false,
  description,
  children,
  image,
  checkbox,
  actions,
  divider = false,
  onClose,
  dismissible = true,
  closeButton = true,
  closeLabel = 'Close',
  returnFocusRef,
  initialFocusRef,
  className,
  'aria-describedby': describedBy,
  ...rest
}: ModalProps) {
  const ref = useRef<HTMLDialogElement>(null)
  const pressedOutside = useRef(false)
  const titleId = useId()
  const descriptionId = useId()
  const dismiss = () => {
    if (dismissible) onClose()
  }

  useEffect(() => {
    const dialog = ref.current
    if (!dialog) return
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const returnTo = returnFocusRef?.current ?? opener
    const root = document.documentElement
    const { overflow, paddingRight } = root.style
    // The scrollbar's width becomes padding, so hiding it does not shift the page.
    const scrollbar = window.innerWidth - root.clientWidth
    root.style.overflow = 'hidden'
    if (scrollbar > 0) root.style.paddingRight = `${scrollbar}px`
    show(dialog)
    initialFocusRef?.current?.focus()
    return () => {
      hide(dialog)
      root.style.overflow = overflow
      root.style.paddingRight = paddingRight
      if (returnTo?.isConnected) returnTo.focus()
    }
  }, [returnFocusRef, initialFocusRef])

  const describedByIds = [description != null ? descriptionId : undefined, describedBy]
    .filter(Boolean)
    .join(' ')

  return (
    <dialog
      {...rest}
      ref={ref}
      className={[styles.root, className].filter(Boolean).join(' ')}
      aria-labelledby={titleId}
      aria-describedby={describedByIds || undefined}
      onKeyDown={(event) => {
        // A control inside that already used the key (an open help text) keeps the dialog open.
        if (event.key !== 'Escape' || event.defaultPrevented) return
        event.preventDefault()
        dismiss()
      }}
      onCancel={(event) => {
        event.preventDefault()
        dismiss()
      }}
      // A close the browser forces (a repeated Escape, a back gesture) cannot be refused: show the
      // dialog again while it must stay open, otherwise let the caller follow.
      onClose={() => {
        const dialog = ref.current
        // The event arrives a task later: a dialog already shown again has nothing to follow.
        if (!dialog || dialog.open) return
        if (dismissible) onClose()
        else show(dialog)
      }}
      onPointerDown={(event) => {
        pressedOutside.current =
          event.target === event.currentTarget &&
          isOutside(event.currentTarget, event.clientX, event.clientY)
      }}
      onClick={(event) => {
        // Only a press and a release both on the backdrop: a drag out of the dialog keeps it open.
        const fromOutside = pressedOutside.current
        pressedOutside.current = false
        if (event.target !== event.currentTarget || !fromOutside) return
        if (isOutside(event.currentTarget, event.clientX, event.clientY)) dismiss()
      }}
    >
      {closeButton ? (
        <CloseButton
          variant="plain"
          className={styles.close}
          aria-label={closeLabel}
          onClick={dismiss}
          disabled={!dismissible}
        />
      ) : null}
      {image != null ? <div className={styles.image}>{image}</div> : null}
      <div className={styles.body}>
        <div className={styles.text}>
          <h2 id={titleId} className={styles.title} data-hidden={titleHidden || undefined}>
            {title}
          </h2>
          {description != null ? (
            <div id={descriptionId} className={styles.description}>
              {description}
            </div>
          ) : null}
        </div>
        {children != null ? <div className={styles.content}>{children}</div> : null}
        {checkbox ? (
          <div className={styles.checkbox}>
            <CheckboxLabel {...checkbox} />
          </div>
        ) : null}
      </div>
      {actions != null ? (
        <div className={styles.actions}>
          {divider ? <hr className={styles.divider} /> : null}
          <div className={styles.buttons}>{actions}</div>
        </div>
      ) : null}
    </dialog>
  )
}
