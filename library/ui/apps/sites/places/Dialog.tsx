import { useEffect, useRef, type KeyboardEvent, type ReactNode } from 'react'
import styles from './Dialog.module.css'

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'

type DialogProps = {
  /** The dialog's accessible name. */
  label: string
  onClose: () => void
  /** `modal` centers the panel; `drawer` slides it in from the right edge. */
  placement?: 'modal' | 'drawer'
  className?: string
  children: ReactNode
}

/**
 * TODO(figma-ds): a minimal modal dialog until the design system ships one. Mounted means open:
 * focus moves inside and stays there, Escape and the backdrop close it, the page behind does not
 * scroll, and focus returns to what opened it.
 */
function Dialog({ label, onClose, placement = 'modal', className, children }: DialogProps) {
  const panel = useRef<HTMLDivElement>(null)
  const close = useRef(onClose)
  useEffect(() => {
    close.current = onClose
  })

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const node = panel.current
    ;(node?.querySelector<HTMLElement>(FOCUSABLE) ?? node)?.focus()
    const overflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    return () => {
      document.body.style.overflow = overflow
      if (opener?.isConnected) opener.focus()
    }
  }, [])

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') close.current()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [])

  const trap = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'Tab' || !panel.current) return
    const items = [...panel.current.querySelectorAll<HTMLElement>(FOCUSABLE)]
    if (!items.length) {
      event.preventDefault()
      return
    }
    const first = items[0]!
    const last = items[items.length - 1]!
    const active = document.activeElement
    if (event.shiftKey && (active === first || active === panel.current)) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && active === last) {
      event.preventDefault()
      first.focus()
    }
  }

  return (
    <div
      className={`${styles.backdrop} ${placement === 'drawer' ? styles.drawerBackdrop : ''}`}
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose()
      }}
    >
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        tabIndex={-1}
        className={[styles.panel, placement === 'drawer' ? styles.drawer : styles.modal, className]
          .filter(Boolean)
          .join(' ')}
        onKeyDown={trap}
      >
        {children}
      </div>
    </div>
  )
}

export { Dialog }
export type { DialogProps }
