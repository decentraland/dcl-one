import { createContext, useContext, useEffect, useRef, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { portalRoot } from '../../portalRoot'
import { CloseIcon } from '../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { track } from '@/data/lib/creators/analytics'
import styles from './Modal.module.css'

// Refcounted scroll lock: modals can stack (add-items + its confirm dialogs), and a
// last-writer-wins restore would leave the page locked after closing both. The lock goes on <html>:
// with `overflow-x: clip` on the root, a body overflow no longer propagates to the viewport.
// `savedOverflow` is captured only when the first modal opens: nothing else touches the root
// overflow while a modal is up, so a value set mid-stack would be clobbered on release.
let scrollLocks = 0
let savedOverflow = ''

// Every modal listens on the document, so without a notion of "topmost" a parent would treat focus
// inside a stacked child as "outside" and drag it back on every Tab. Topmost is the deepest dialog
// in the React tree (nesting depth via context) and, among equals, the latest mounted; neither
// depends on effect order, which runs children before parents.
type OpenDialog = { element: HTMLElement; depth: number; order: number }
const openDialogs: OpenDialog[] = []
let mountCounter = 0
const ModalDepthContext = createContext(0)

function topmostDialog(): HTMLElement | undefined {
  return openDialogs.reduce<OpenDialog | undefined>(
    (top, entry) =>
      !top || entry.depth > top.depth || (entry.depth === top.depth && entry.order > top.order) ? entry : top,
    undefined
  )?.element
}

function acquireScrollLock() {
  if (scrollLocks === 0) {
    savedOverflow = document.documentElement.style.overflow
    document.documentElement.style.overflow = 'hidden'
  }
  scrollLocks++
}

function releaseScrollLock() {
  scrollLocks--
  if (scrollLocks === 0) {
    document.documentElement.style.overflow = savedOverflow
  }
}

type Props = {
  title: string
  onClose: () => void
  children: ReactNode
  /** Blocks every close affordance (✕, Escape, scrim) while a submit is in flight. */
  closeDisabled?: boolean
  /** 'medium' for success dialogs, 'large' for wizards, 'wide' for editor-style dialogs (add items); default is the 560px form dialog. */
  size?: 'default' | 'medium' | 'large' | 'wide'
  /** Renders no title bar (confirm/error dialogs); `title` still labels the dialog for a11y. */
  hideTitle?: boolean
  /** With `hideTitle`, still shows a floating ✕ in the dialog corner. */
  showClose?: boolean
  /** Drops the ✕ from the title bar, for work that can't be interrupted. */
  hideClose?: boolean
  /** Tighter dialog padding for small form dialogs (sell, send, update price). */
  compact?: boolean
  /** Removes the dialog padding so children can draw edge-to-edge panes; the title bar keeps its own. */
  flush?: boolean
  testId?: string
}

export function Modal({
  title,
  onClose,
  children,
  closeDisabled = false,
  size = 'default',
  hideTitle = false,
  showClose = false,
  hideClose = false,
  compact = false,
  flush = false,
  testId = 'modal'
}: Props) {
  const dialogRef = useRef<HTMLDivElement>(null)
  const depth = useContext(ModalDepthContext)

  // Latest-value refs so the document-level listeners never rebind mid-interaction.
  const closeRef = useRef(onClose)
  closeRef.current = onClose
  const closeDisabledRef = useRef(closeDisabled)
  closeDisabledRef.current = closeDisabled

  useEffect(() => {
    const dialog = dialogRef.current
    const entry: OpenDialog | null = dialog ? { element: dialog, depth, order: ++mountCounter } : null
    if (entry) openDialogs.push(entry)
    dialog?.focus()

    track('bd_modal_opened', { name: testId })

    function onKeyDown(event: KeyboardEvent) {
      if (topmostDialog() !== dialog) return
      if (event.key === 'Escape' && !closeDisabledRef.current) closeRef.current()

      // aria-modal alone doesn't stop Tab from reaching the page behind the scrim.
      if (event.key !== 'Tab' || !dialogRef.current) return
      const focusables = dialogRef.current.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
      )
      if (focusables.length === 0) {
        event.preventDefault()
        return
      }
      const first = focusables[0]
      const last = focusables[focusables.length - 1]
      const active = document.activeElement
      const isInside = dialogRef.current.contains(active)
      if (event.shiftKey && (active === first || !isInside)) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && (active === last || !isInside)) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', onKeyDown)

    // The page must not scroll behind the scrim.
    acquireScrollLock()

    return () => {
      document.removeEventListener('keydown', onKeyDown)
      if (entry) openDialogs.splice(openDialogs.indexOf(entry), 1)
      releaseScrollLock()
      // Hand focus back to the dialog underneath, if any.
      topmostDialog()?.focus()
      track('bd_modal_closed', { name: testId })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const { t } = useTranslation()

  const closeButton = (
    <button
      className={styles.closeButton}
      type="button"
      aria-label={t('modal.close')}
      data-testid={`${testId}-close`}
      disabled={closeDisabled}
      onClick={onClose}
    >
      <CloseIcon fontSize="small" />
    </button>
  )

  return createPortal(
    <div className={styles.scrim} data-testid={`${testId}-scrim`} onClick={() => !closeDisabled && onClose()}>
      <div
        className={styles.dialog}
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        data-testid={testId}
        data-size={size}
        data-compact={compact || undefined}
        data-flush={flush || undefined}
        onClick={event => event.stopPropagation()}
      >
        {!hideTitle && (
          <div className={styles.titleBar}>
            <h2 className={styles.title}>{title}</h2>
            {!hideClose && closeButton}
          </div>
        )}
        {hideTitle && showClose && <div className={styles.floatingClose}>{closeButton}</div>}
        <div className={styles.body} data-titleless={hideTitle || undefined}>
          <ModalDepthContext.Provider value={depth + 1}>{children}</ModalDepthContext.Provider>
        </div>
      </div>
    </div>,
    portalRoot()
  )
}
