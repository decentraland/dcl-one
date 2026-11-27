import { useEffect, useRef, type ReactNode } from 'react'
import styles from './OptionMenu.module.css'

export interface OptionSection<T extends string = string> {
  label: string
  options: readonly { id: T; label: string }[]
  value: T
  onChange: (id: T) => void
}

export interface OptionMenuProps {
  sections: readonly OptionSection[]
  /** A press outside the list and the element that holds it (the button that toggles it sits there). */
  onClose: () => void
  /** The mark of a chosen option, 14px; it takes the check's colour. A plain check without it. */
  check?: ReactNode
  /**
   * Called as the list mounts, with a function that closes it; what it returns runs as the list leaves. A host
   * registers the list there with whatever closes its layers.
   */
  whileOpen?: (close: () => void) => () => void
  className?: string
}

/** A light list of single-choice sections (sort by, view), each chosen option marked with a check. */
export function OptionMenu({ sections, onClose, check, whileOpen, className }: OptionMenuProps) {
  const root = useRef<HTMLDivElement>(null)
  const opened = useRef(whileOpen)
  useEffect(() => {
    opened.current = whileOpen
  })
  useEffect(() => {
    const press = (event: MouseEvent): void => {
      const target = event.target as Node
      const node = root.current
      if (node != null && !node.contains(target) && !(node.parentElement?.contains(target) ?? false)) onClose()
    }
    document.addEventListener('mousedown', press)
    const off = opened.current?.(onClose)
    return () => {
      document.removeEventListener('mousedown', press)
      off?.()
    }
  }, [onClose])
  return (
    <div ref={root} className={[styles.menu, className].filter(Boolean).join(' ')} role="menu">
      {sections.map((section, index) => (
        <div key={section.label} className={styles.section}>
          {index > 0 && <span className={styles.separator} aria-hidden="true" />}
          <span className={styles.label}>{section.label}</span>
          {section.options.map((option) => (
            <button
              key={option.id}
              type="button"
              role="menuitemradio"
              aria-checked={option.id === section.value}
              className={styles.option}
              onClick={() => section.onChange(option.id)}
            >
              {option.label}
              {option.id === section.value && (
                <span className={styles.check} aria-hidden="true">
                  {check ?? <CheckMark />}
                </span>
              )}
            </button>
          ))}
        </div>
      ))}
    </div>
  )
}

function CheckMark() {
  return (
    <svg
      viewBox="0 0 14 14"
      width="14"
      height="14"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M2.5 7.5l3 3 6-7" />
    </svg>
  )
}
