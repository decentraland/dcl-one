import { useCallback, useEffect, useId, useRef, useState, useSyncExternalStore } from 'react'
import { createPortal } from 'react-dom'
import type { HTMLAttributes, KeyboardEvent, ReactNode } from 'react'
import styles from './Modal.module.css'

type ModalProps = {
  title: ReactNode
  // Line under the title, e.g. a balance. Its content belongs to the application.
  subtitle?: ReactNode
  // Called by the close button and by Escape.
  onClose?: () => void
  closeLabel?: string
  // Buttons slot: every action gets the same width.
  actions?: ReactNode
  children?: ReactNode
} & Omit<HTMLAttributes<HTMLDivElement>, 'title'>

function CloseIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path d="M6 6l12 12M18 6L6 18" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  )
}

function useHasMoreBelow() {
  const ref = useRef<HTMLDivElement>(null)
  const [hasMoreBelow, setHasMoreBelow] = useState(false)

  const measure = useCallback(() => {
    const node = ref.current
    if (!node) return
    setHasMoreBelow(node.scrollTop + node.clientHeight < node.scrollHeight - 1)
  }, [])

  useEffect(() => {
    measure()
    const node = ref.current
    if (!node || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(measure)
    observer.observe(node)
    for (const child of node.children) observer.observe(child)
    return () => observer.disconnect()
  }, [measure])

  return { ref, hasMoreBelow, onScroll: measure }
}

const FOCUSABLE =
  'a[href], area[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), iframe, [contenteditable="true"], [tabindex]:not([tabindex="-1"])'

function focusableIn(container: HTMLElement): HTMLElement[] {
  return [...container.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (element) => !element.hasAttribute('inert') && element.getAttribute('aria-hidden') !== 'true'
  )
}

// A modal dialog keeps focus inside itself: initial focus on its first focusable element (or the
// dialog when it has none), Tab and Shift+Tab wrap, focus moved outside is pulled back, and the
// opener gets focus back when the dialog closes.
function useFocusTrap(mountKey: unknown) {
  const ref = useRef<HTMLDivElement>(null)
  // Re-runs when the panel is re-mounted (moved into the portal after hydration).
  useEffect(() => {
    const dialog = ref.current
    if (!dialog) return
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    ;(focusableIn(dialog)[0] ?? dialog).focus()
    const pullBack = (event: FocusEvent) => {
      if (event.target instanceof Node && !dialog.contains(event.target))
        (focusableIn(dialog)[0] ?? dialog).focus()
    }
    document.addEventListener('focusin', pullBack)
    return () => {
      document.removeEventListener('focusin', pullBack)
      opener?.focus()
    }
  }, [mountKey])
  return ref
}

const subscribeNever = () => () => {}

/**
 * False on the server and while hydrating (so the markup matches the server's), true afterwards and
 * on client-only renders.
 */
function useHydrated(): boolean {
  return useSyncExternalStore(
    subscribeNever,
    () => true,
    () => false
  )
}

function trapTab(event: KeyboardEvent<HTMLDivElement>) {
  const focusable = focusableIn(event.currentTarget)
  if (focusable.length === 0) {
    event.preventDefault()
    return
  }
  const first = focusable[0]!
  const last = focusable[focusable.length - 1]!
  const active = document.activeElement
  if (event.shiftKey && (active === first || active === event.currentTarget)) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && active === last) {
    event.preventDefault()
    first.focus()
  }
}

function Modal({
  title,
  subtitle,
  onClose,
  closeLabel = 'Close',
  actions,
  children,
  className,
  onKeyDown,
  ...rest
}: ModalProps) {
  const titleId = useId()
  const { ref, hasMoreBelow, onScroll } = useHasMoreBelow()
  const hydrated = useHydrated()
  const panelRef = useFocusTrap(hydrated)

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    onKeyDown?.(event)
    if (event.defaultPrevented) return
    if (event.key === 'Tab') trapTab(event)
    else if (event.key === 'Escape' && onClose) {
      event.stopPropagation()
      onClose()
    }
  }

  const dialog = (
    <div
      ref={panelRef}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      tabIndex={-1}
      className={[styles.panel, className].filter(Boolean).join(' ')}
      onKeyDown={handleKeyDown}
      {...rest}
    >
      <header className={styles.topBar}>
        <div className={styles.titleRow}>
          <h2 id={titleId} className={styles.title}>
            {title}
          </h2>
          {onClose && (
            <button
              type="button"
              className={styles.close}
              aria-label={closeLabel}
              onClick={onClose}
            >
              <CloseIcon />
            </button>
          )}
        </div>
        {subtitle !== undefined && <div className={styles.subtitle}>{subtitle}</div>}
      </header>
      <div className={styles.contentFrame}>
        <div ref={ref} className={styles.content} onScroll={onScroll}>
          {children}
        </div>
        {hasMoreBelow && <div className={styles.scrollMask} />}
      </div>
      {actions !== undefined && <footer className={styles.actions}>{actions}</footer>}
    </div>
  )

  // A modal covers the page: a fixed backdrop on document.body, so no ancestor's overflow or
  // stacking context clips it. The server (and hydration, to match it) renders it in place first.
  const overlay = <div className={styles.backdrop}>{dialog}</div>
  return hydrated ? createPortal(overlay, document.body) : overlay
}

export { Modal }
export type { ModalProps }
