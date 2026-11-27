import { useEffect, useRef, useState, type ReactNode } from 'react'
import { GeneralKeyboard } from '../../foundations/icons/general'
import styles from './HintsButton.module.css'

export type Hint = {
  /** A mark up to 20px wide, for example a KeyCap. */
  icon: ReactNode
  text: string
}

export type HintsButtonProps = {
  hints: readonly Hint[]
  /** The accessible name of the button and of the list it opens. */
  label?: string
  /** The button's 22px mark; it takes the button's colour. A keyboard without it. */
  icon?: ReactNode
  /**
   * Called as the list opens, with a function that closes it; what it returns runs as the list leaves. A host
   * registers the list there with whatever closes its layers.
   */
  whileOpen?: (close: () => void) => () => void
  className?: string
}

/**
 * A dark square button that opens a list of control hints above it, each a mark and a line of text. A press
 * outside closes the list; it handles no Escape, which the host does through `whileOpen`.
 */
export function HintsButton({
  hints,
  label = 'Controls',
  icon,
  whileOpen,
  className
}: HintsButtonProps) {
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  const opened = useRef(whileOpen)
  useEffect(() => {
    opened.current = whileOpen
  })
  useEffect(() => {
    if (!open) return
    const close = (): void => setOpen(false)
    const press = (event: MouseEvent): void => {
      if (root.current != null && !root.current.contains(event.target as Node)) close()
    }
    document.addEventListener('mousedown', press)
    const off = opened.current?.(close)
    return () => {
      document.removeEventListener('mousedown', press)
      off?.()
    }
  }, [open])
  return (
    <div ref={root} className={[styles.root, className].filter(Boolean).join(' ')}>
      <button
        type="button"
        className={styles.button}
        aria-label={label}
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        {icon ?? <GeneralKeyboard size={22} aria-hidden="true" />}
      </button>
      {open && (
        <ul className={styles.popover} aria-label={label}>
          {hints.map((hint) => (
            <li key={hint.text} className={styles.row}>
              <span className={styles.icon}>{hint.icon}</span>
              {hint.text}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

/** The outline of a key with its label, for a keyboard hint. */
export function KeyCap({ label }: { label: string }) {
  return <span className={styles.keyCap}>{label}</span>
}
