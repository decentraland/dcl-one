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
  | 'onPointerDown'
  | 'onClick'
  | 'aria-labelledby'
> & {
  title: ReactNode
  /**
   * `gallery` fills the screen on a phone and is a centered media surface on a desktop. `detail` is
   * an 880px card for an event or place detail: `image` is the cover, edge to edge on top, and the
   * children follow without padding; on a phone it is a full-height sheet below the 64px site
   * header. `scene` is the same card on the purple scene gradient with 24px corners and a glow,
   * 680px to 880px wide with the viewport (decentraland.org's place card). `gallery`, `detail` and
   * `scene` keep the title out of view, so the content draws its own visible heading.
   */
  layout?: 'message' | 'gallery' | 'detail' | 'scene'
  /**
   * `scene` only: the dialog is the page's main content, such as a place's own page. It is
   * server-rendered shown, so it paints before the page hydrates, its title is the page's `h1`, and
   * on a phone (below 600px) it sits in the page flow, without a backdrop or a scroll lock.
   */
  pageContent?: boolean
  /** Keeps the title out of view. It still names the dialog. */
  titleHidden?: boolean
  /** The body text. It is the dialog's description. */
  description?: ReactNode
  children?: ReactNode
  /**
   * A picture or illustration above the title. A decorative one has an empty `alt`. In `detail`
   * and `scene` it is the cover, and may carry the text and actions drawn over it.
   */
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
  /**
   * `false` keeps the dialog open on a press on the backdrop, so a stray click does not discard a
   * form; Escape and the close button still close it. Defaults to `true`.
   */
  closeOnBackdrop?: boolean
  /** Shows the close button in the corner. Defaults to `true`; without it an action must close the dialog. */
  closeButton?: boolean
  closeLabel?: string
  /** Where focus goes when the dialog closes. Defaults to the element that had focus when it opened. */
  returnFocusRef?: RefObject<HTMLElement | null>
  /** The element that takes focus when the dialog opens. Defaults to the first control inside. */
  initialFocusRef?: RefObject<HTMLElement | null>
}

// The width below which a `pageContent` dialog sits in the page instead of above it. Modal.module.css
// repeats it for the in-page and pre-hydration styles; change the three together.
const PAGE_ON_PHONE = '(max-width: 599.95px)'

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

// Shown in the page flow: not modal, so the page stays interactive and scrolls.
function showInPage(dialog: HTMLDialogElement) {
  if (dialog.open) return
  if (typeof dialog.show === 'function') dialog.show()
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
  layout = 'message',
  pageContent = false,
  titleHidden = false,
  description,
  children,
  image,
  checkbox,
  actions,
  divider = false,
  onClose,
  dismissible = true,
  closeOnBackdrop = true,
  closeButton = true,
  closeLabel = 'Close',
  returnFocusRef,
  initialFocusRef,
  className,
  onKeyDown,
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

  const inPage = pageContent && layout === 'scene'
  const detailed = layout === 'detail' || layout === 'scene'
  // A page's own card names the page too, so its title is the page's main heading.
  const Heading = inPage ? 'h1' : 'h2'
  const heading = (
    <Heading
      id={titleId}
      className={styles.title}
      data-hidden={titleHidden || layout !== 'message' || undefined}
    >
      {title}
    </Heading>
  )

  useEffect(() => {
    const dialog = ref.current
    if (!dialog) return
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const returnTo = returnFocusRef?.current ?? opener
    const root = document.documentElement
    const { overflow, paddingRight } = root.style
    let modal = false
    const unlock = () => {
      root.style.overflow = overflow
      root.style.paddingRight = paddingRight
    }
    const phone =
      inPage && typeof window.matchMedia === 'function' ? window.matchMedia(PAGE_ON_PHONE) : null
    // A page dialog arrives shown in the page (server-rendered `open`); above a phone it is reopened modal.
    const place = () => {
      if (phone?.matches) {
        if (modal) hide(dialog)
        modal = false
        unlock()
        showInPage(dialog)
        return
      }
      if (modal) return
      if (inPage) hide(dialog)
      modal = true
      // The scrollbar's width becomes padding, so hiding it does not shift the page.
      const scrollbar = window.innerWidth - root.clientWidth
      root.style.overflow = 'hidden'
      if (scrollbar > 0) root.style.paddingRight = `${scrollbar}px`
      show(dialog)
      initialFocusRef?.current?.focus()
    }
    place()
    phone?.addEventListener('change', place)
    return () => {
      phone?.removeEventListener('change', place)
      hide(dialog)
      unlock()
      if (returnTo?.isConnected) returnTo.focus()
    }
  }, [returnFocusRef, initialFocusRef, inPage])

  const describedByIds = [description != null ? descriptionId : undefined, describedBy]
    .filter(Boolean)
    .join(' ')

  return (
    <dialog
      {...rest}
      ref={ref}
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-layout={layout === 'message' ? undefined : layout}
      data-page={inPage || undefined}
      // A page dialog is server-rendered shown; the effect then decides between the page and the top layer.
      open={inPage || undefined}
      aria-labelledby={titleId}
      aria-describedby={describedByIds || undefined}
      onKeyDown={(event) => {
        onKeyDown?.(event)
        if (event.defaultPrevented) return
        if (event.key === 'Escape') {
          event.preventDefault()
          dismiss()
        } else if (event.key === 'Tab' && typeof event.currentTarget.showModal !== 'function') {
          // A browser without showModal still needs to wrap focus between the visible controls.
          const controls = event.currentTarget.querySelectorAll<HTMLElement>(
            'a[href], button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])'
          )
          const first = controls[0]
          const last = controls[controls.length - 1]
          const edge = event.shiftKey ? first : last
          const wrapTo = event.shiftKey ? last : first
          if (edge && document.activeElement === edge && wrapTo) {
            event.preventDefault()
            wrapTo.focus()
          }
        }
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
        if (!closeOnBackdrop || event.target !== event.currentTarget || !fromOutside) return
        if (isOutside(event.currentTarget, event.clientX, event.clientY)) dismiss()
      }}
    >
      {closeButton ? (
        <CloseButton
          variant={detailed ? 'filled' : 'plain'}
          className={styles.close}
          aria-label={closeLabel}
          onClick={dismiss}
          disabled={!dismissible}
        />
      ) : null}
      {/* A cover carries its own visible title, so the hidden one is read first, before it. */}
      {detailed ? heading : null}
      {image != null ? <div className={styles.image}>{image}</div> : null}
      <div className={styles.body}>
        {detailed && description == null ? null : (
          <div className={styles.text}>
            {detailed ? null : heading}
            {description != null ? (
              <div id={descriptionId} className={styles.description}>
                {description}
              </div>
            ) : null}
          </div>
        )}
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
