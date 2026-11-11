import { useEffect, useRef, type ReactNode } from 'react'
import styles from './Dialog.module.css'

// TODO(figma-ds): a minimal modal dialog until the design system ships one: focus moves in and is
// kept inside, Escape and the backdrop close it, focus returns to the opener, the page stops scrolling.

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select, textarea, [tabindex]:not([tabindex="-1"])'

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
      const items = [...panel.current.querySelectorAll<HTMLElement>(FOCUSABLE)]
      const first = items[0]
      const last = items[items.length - 1]
      if (!first || !last) {
        event.preventDefault()
        panel.current.focus()
        return
      }
      const active = document.activeElement
      if (event.shiftKey && (active === first || active === panel.current)) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && active === last) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('keydown', onKeyDown)
      document.body.style.overflow = overflow
      opener?.focus()
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
