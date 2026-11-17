import { useEffect, useRef, type ReactNode } from 'react'
import styles from './Dialog.module.css'

// TODO(figma-ds): a minimal modal dialog until the design system ships one: focus moves in and is
// kept inside, Escape and the backdrop close it, focus returns to the opener, the page stops scrolling.

const FOCUSABLE = [
  'a[href]',
  'area[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'summary',
  'iframe',
  'audio[controls]',
  'video[controls]',
  '[contenteditable]:not([contenteditable="false"])',
  '[tabindex]:not([tabindex="-1"])'
].join(',')

/** Hidden, inert or explicitly unfocusable elements are not tab stops. */
function tabbable(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (el) =>
      el.tabIndex >= 0 &&
      !el.closest('[inert], [hidden], [aria-hidden="true"]') &&
      getComputedStyle(el).visibility !== 'hidden' &&
      getComputedStyle(el).display !== 'none'
  )
}

function Dialog({
  labelledBy,
  onClose,
  children
}: {
  labelledBy: string
  onClose: () => void
  children: ReactNode
}) {
  const panel = useRef<HTMLDivElement | null>(null)
  const close = useRef(onClose)
  close.current = onClose

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const overflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    panel.current?.focus()
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        close.current()
        return
      }
      if (event.key !== 'Tab' || !panel.current) return
      // The dialog moves focus itself, so a stop the browser would reach (an inert or hidden one) never is.
      event.preventDefault()
      const items = tabbable(panel.current)
      if (!items.length) {
        panel.current.focus()
        return
      }
      const at = items.indexOf(document.activeElement as HTMLElement)
      const step = event.shiftKey ? -1 : 1
      const next =
        at === -1
          ? event.shiftKey
            ? items.length - 1
            : 0
          : (at + step + items.length) % items.length
      items[next]?.focus()
    }
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('keydown', onKeyDown)
      document.body.style.overflow = overflow
      // The opener may have left the page meanwhile (a list re-rendered); focusing it then does nothing useful.
      if (opener?.isConnected) opener.focus()
    }
  }, [])

  return (
    <div
      className={styles.backdrop}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose()
      }}
    >
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
        tabIndex={-1}
        className={styles.panel}
      >
        {children}
      </div>
    </div>
  )
}

export { Dialog }
