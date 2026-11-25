import { useEffect, useRef, useState } from 'react'
import styles from './Dropdown.module.css'

export type HudDropdownOption = {
  value: string
  label: string
}

/**
 * Not in Figma: the Explorer HUD's select. A field that opens a list of option buttons below it, or above it
 * when the box that would clip the list has no room below.
 */
export type HudDropdownProps = {
  appearance: 'hud'
  value: string
  options: readonly HudDropdownOption[]
  onChange: (value: string) => void
  disabled?: boolean
  /** `light` is the off-white field of a solid panel. */
  variant?: 'dark' | 'light'
  /** The field's accessible name. */
  label?: string
  /** Ids of elements that describe the field. */
  'aria-describedby'?: string
  /** Shown in the field while no option has the value. */
  placeholder?: string
}

/** Roughly how tall the list can get (its `max-height` in the stylesheet). */
const LIST_MAX = 260

/**
 * Whether the list opens upwards: only when the room below, inside whatever box would clip it, cannot hold it
 * and there is more room above.
 */
export function preferUp(space: { above: number; below: number; list: number }): boolean {
  return space.below < space.list && space.above > space.below
}

/** The nearest ancestor that would clip the list (a scrolling panel, a dialog), or the viewport. */
function clipBounds(element: HTMLElement): { top: number; bottom: number } {
  for (let node = element.parentElement; node != null; node = node.parentElement) {
    const { overflow, overflowY } = getComputedStyle(node)
    if (
      `${overflow} ${overflowY}`
        .split(' ')
        .some((value) => value === 'auto' || value === 'scroll' || value === 'hidden' || value === 'clip')
    ) {
      const box = node.getBoundingClientRect()
      return { top: box.top, bottom: box.bottom }
    }
  }
  return { top: 0, bottom: window.innerHeight }
}

export function HudDropdown({
  value,
  options,
  onChange,
  disabled = false,
  variant = 'dark',
  label,
  'aria-describedby': describedBy,
  placeholder
}: Omit<HudDropdownProps, 'appearance'>) {
  const [open, setOpen] = useState(false)
  const [up, setUp] = useState(false)
  const root = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const press = (event: MouseEvent) => {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', press)
    return () => document.removeEventListener('mousedown', press)
  }, [open])

  const current = options.find((option) => option.value === value)

  return (
    <div className={styles.hudRoot} ref={root}>
      <button
        type="button"
        className={[styles.hudField, variant === 'light' && styles.hudLight].filter(Boolean).join(' ')}
        disabled={disabled}
        aria-label={label}
        aria-describedby={describedBy}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => {
          if (!open && root.current != null) {
            const box = root.current.getBoundingClientRect()
            const clip = clipBounds(root.current)
            setUp(preferUp({ above: box.top - clip.top, below: clip.bottom - box.bottom, list: LIST_MAX }))
          }
          setOpen((now) => !now)
        }}
      >
        <span className={styles.hudValue}>{current?.label ?? placeholder ?? value}</span>
        <svg
          className={[styles.hudChevron, open && styles.hudChevronOpen].filter(Boolean).join(' ')}
          viewBox="0 0 12 12"
          aria-hidden="true"
        >
          <path
            d="M2.5 4.5L6 8l3.5-3.5"
            stroke="currentColor"
            strokeWidth="1.6"
            fill="none"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>
      {open && (
        <ul className={[styles.hudList, up && styles.hudListUp].filter(Boolean).join(' ')} role="listbox">
          {options.map((option) => (
            <li key={option.value}>
              <button
                type="button"
                role="option"
                aria-selected={option.value === value}
                className={[styles.hudOption, option.value === value && styles.hudOptionActive]
                  .filter(Boolean)
                  .join(' ')}
                onClick={() => {
                  onChange(option.value)
                  setOpen(false)
                }}
              >
                {option.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
