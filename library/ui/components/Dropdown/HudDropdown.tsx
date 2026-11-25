import { useEffect, useId, useRef, useState } from 'react'
import type { KeyboardEvent } from 'react'
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
  kind?: 'field'
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
}: Omit<HudDropdownProps, 'appearance' | 'kind'>) {
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

/**
 * Not in Figma: the Explorer HUD's sort list. A compact field over a list of plain text options, controlled or
 * not, with the arrow keys, Home, End, Enter and Space.
 */
export type HudSortDropdownProps = {
  appearance: 'hud'
  kind: 'sort'
  options: readonly string[]
  value?: string
  defaultValue?: string
  onChange?: (value: string) => void
  /**
   * Called when the list opens, with a function that closes it; what it returns runs when the list closes. The
   * HUD registers the list there as a layer its Cancel action closes.
   */
  whileOpen?: (close: () => void) => () => void
}

export function HudSortDropdown({
  options,
  value,
  defaultValue,
  onChange,
  whileOpen
}: Omit<HudSortDropdownProps, 'appearance' | 'kind'>) {
  const [internal, setInternal] = useState(defaultValue ?? options[0])
  const controlled = value !== undefined
  const current = controlled ? value : internal
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(-1)
  const root = useRef<HTMLDivElement>(null)
  const field = useRef<HTMLButtonElement>(null)
  const id = useId()
  const opened = useRef(whileOpen)
  useEffect(() => {
    opened.current = whileOpen
  })

  useEffect(() => {
    if (!open) return
    const press = (event: MouseEvent) => {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', press)
    return () => document.removeEventListener('mousedown', press)
  }, [open])

  // The list opens on the chosen option; the options and the value are read once, as it opens.
  const openedOn = useRef({ options, current })
  useEffect(() => {
    openedOn.current = { options, current }
  })
  useEffect(() => {
    if (open) setActive(Math.max(0, openedOn.current.options.indexOf(openedOn.current.current ?? '')))
  }, [open])

  useEffect(() => {
    if (!open) return
    return opened.current?.(() => setOpen(false))
  }, [open])

  const pick = (option: string | undefined) => {
    if (option === undefined) return
    if (!controlled) setInternal(option)
    onChange?.(option)
    setOpen(false)
    field.current?.focus()
  }

  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const { key } = event
    if (key === 'Enter' || key === ' ') {
      event.preventDefault()
      if (!open) setOpen(true)
      else if (active >= 0) pick(options[active])
      return
    }
    if (key === 'ArrowDown' || key === 'ArrowUp' || key === 'Home' || key === 'End') {
      event.preventDefault()
      if (!open) {
        setOpen(true)
        return
      }
      if (key === 'Home') setActive(0)
      else if (key === 'End') setActive(options.length - 1)
      else if (key === 'ArrowDown') setActive((at) => Math.min(options.length - 1, at + 1))
      else setActive((at) => Math.max(0, at - 1))
    }
  }

  return (
    <div
      className={[styles.hudSort, open && styles.hudSortOpen].filter(Boolean).join(' ')}
      ref={root}
      onKeyDown={keyDown}
    >
      <button
        type="button"
        className={styles.hudSortField}
        ref={field}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-activedescendant={open && active >= 0 ? `${id}-${active}` : undefined}
        onClick={() => setOpen((now) => !now)}
      >
        <span>{current}</span>
        <svg viewBox="0 0 12 8" width="11" height="8" aria-hidden="true" className={styles.hudSortCaret}>
          <path
            d="M1 1.5L6 6.5l5-5"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>
      {open && (
        <ul className={styles.hudSortMenu} role="listbox">
          {options.map((option, index) => (
            <li
              key={option}
              id={`${id}-${index}`}
              role="option"
              aria-selected={option === current}
              className={[styles.hudSortOption, option === current && styles.hudSortOptionActive]
                .filter(Boolean)
                .join(' ')}
              onClick={() => pick(option)}
            >
              {option}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
