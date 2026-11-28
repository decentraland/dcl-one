import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import type { HTMLAttributes, KeyboardEvent, ReactElement, RefObject } from 'react'
import { GeneralInfoButton } from '../../foundations/icons/general'
import {
  HudDropdown,
  HudSortDropdown,
  type HudDropdownProps,
  type HudSortDropdownProps
} from './HudDropdown'
import styles from './Dropdown.module.css'

export type DropdownOption<T extends string = string> = {
  value: T
  label: string
  disabled?: boolean
}

type DropdownCommon<T extends string> = Omit<
  HTMLAttributes<HTMLDivElement>,
  'children' | 'onChange' | 'defaultValue'
> & {
  /** The visible label above the control, which is also its accessible name. */
  label: string
  labelHidden?: boolean
  options: readonly DropdownOption<T>[]
  placeholder?: string
  disabled?: boolean
  /**
   * Shows the help button after the label. The text is the control's description and opens below
   * the button on click, tap, Enter or Space.
   */
  help?: string
  helpLabel?: string
  /** Submits the choice with a form: one hidden field for each chosen value. */
  name?: string
  /**
   * `light` (the default) is the Figma Dropdown. `dark` is not in Figma: a translucent black field and a
   * raised dark list that marks the chosen option with a ruby fill, as the Places filters draw it.
   */
  variant?: 'light' | 'dark'
}

export type DropdownSingleProps<T extends string> = DropdownCommon<T> & {
  multiple?: false
  valueText?: never
  value: T | null
  onChange: (value: T) => void
}

export type DropdownMultipleProps<T extends string> = DropdownCommon<T> & {
  multiple: true
  value: readonly T[]
  /** Selected options follow list order. Selected values absent from the list keep their order at the end. */
  onChange: (value: T[]) => void
  valueText?: (chosen: readonly DropdownOption<T>[]) => string
}

export type DropdownProps<T extends string = string> = (DropdownSingleProps<T> | DropdownMultipleProps<T>) & {
  appearance?: 'default'
}

type WithoutAppearance<P> = P extends unknown ? Omit<P, 'appearance'> : never

const GUTTER = 16

function useDismiss(
  open: boolean,
  root: RefObject<HTMLElement | null>,
  focus: RefObject<HTMLElement | null>,
  close: () => void
) {
  useEffect(() => {
    if (!open) return
    const press = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) close()
    }
    // Escape closes this only: consumed before an enclosing dialog or page handler sees it.
    const escape = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      close()
      focus.current?.focus()
    }
    document.addEventListener('pointerdown', press)
    document.addEventListener('keydown', escape, true)
    return () => {
      document.removeEventListener('pointerdown', press)
      document.removeEventListener('keydown', escape, true)
    }
  }, [open, root, focus])
}

function useInViewport(
  open: boolean,
  popup: RefObject<HTMLElement | null>,
  anchor: RefObject<HTMLElement | null>
) {
  useLayoutEffect(() => {
    if (!open) return
    const place = () => {
      const element = popup.current
      const from = anchor.current
      if (!element || !from) return
      const viewport = document.documentElement
      element.style.setProperty('--dropdown-shift', '0px')
      delete element.dataset.side
      const below = element.getBoundingClientRect()
      const at = from.getBoundingClientRect()
      if (below.bottom > viewport.clientHeight - GUTTER && at.top > viewport.clientHeight - at.bottom)
        element.dataset.side = 'top'
      const room = viewport.clientWidth - GUTTER
      const shift =
        below.right > room ? Math.max(room - below.right, GUTTER - below.left) : below.left < GUTTER ? GUTTER - below.left : 0
      element.style.setProperty('--dropdown-shift', `${shift}px`)
    }
    place()
    window.addEventListener('resize', place)
    return () => window.removeEventListener('resize', place)
  }, [open, popup, anchor])
}

function Help({ id, text, label, onActivate }: { id: string; text: string; label: string; onActivate: () => void }) {
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLSpanElement>(null)
  const button = useRef<HTMLButtonElement>(null)
  const bubble = useRef<HTMLSpanElement>(null)
  useDismiss(open, root, button, () => setOpen(false))
  useInViewport(open, bubble, button)
  return (
    <span
      ref={root}
      className={styles.help}
      onBlur={(event) => {
        if (!root.current?.contains(event.relatedTarget)) setOpen(false)
      }}
    >
      <button
        ref={button}
        type="button"
        className={styles.helpButton}
        aria-label={label}
        aria-expanded={open}
        aria-controls={id}
        onClick={() => {
          onActivate()
          setOpen((now) => !now)
        }}
      >
        <GeneralInfoButton size={18} aria-hidden="true" focusable="false" />
      </button>
      <span ref={bubble} id={id} className={styles.bubble} hidden={!open}>
        {text}
      </span>
    </span>
  )
}

/**
 * A labelled choice from a list, one option or several: a button with the `combobox` role that
 * opens a `listbox`. The focus stays on the button. Arrow keys, Home, End and typed letters move
 * through the options; Enter or Space chooses; Escape, Tab and a press outside close the list.
 *
 * With `appearance="hud"` it is the Explorer HUD's select instead (`HudDropdownProps`): a field that opens a
 * list of option buttons. With `kind="sort"` too, it is the HUD's sort list (`HudSortDropdownProps`): a compact
 * field over plain text options, with keyboard navigation.
 */
export function Dropdown(props: HudSortDropdownProps): ReactElement
export function Dropdown(props: HudDropdownProps): ReactElement
export function Dropdown<T extends string = string>(props: DropdownProps<T>): ReactElement
export function Dropdown<T extends string = string>({
  appearance = 'default',
  ...props
}: DropdownProps<T> | HudDropdownProps | HudSortDropdownProps) {
  if (appearance === 'hud') {
    const { kind = 'field', ...hud } = props as Omit<HudDropdownProps | HudSortDropdownProps, 'appearance'>
    if (kind === 'sort')
      return <HudSortDropdown {...(hud as Omit<HudSortDropdownProps, 'appearance' | 'kind'>)} />
    return <HudDropdown {...(hud as Omit<HudDropdownProps, 'appearance' | 'kind'>)} />
  }
  return <FigmaDropdown {...(props as WithoutAppearance<DropdownProps<T>>)} />
}

function FigmaDropdown<T extends string = string>(props: WithoutAppearance<DropdownProps<T>>) {
  const {
    label,
    labelHidden = false,
    options,
    placeholder = '',
    disabled = false,
    help,
    helpLabel = 'More information',
    name,
    variant = 'light',
    multiple,
    value,
    onChange,
    valueText,
    className,
    ...rest
  } = props
  const id = useId()
  const listId = `${id}-list`
  const labelId = `${id}-label`
  const helpId = `${id}-help`
  const optionId = (index: number) => `${id}-option-${index}`
  const root = useRef<HTMLDivElement>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  const list = useRef<HTMLDivElement>(null)
  const typed = useRef({ text: '', at: 0 })
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(-1)
  if (disabled && open) setOpen(false)

  const chosen: readonly T[] = multiple ? value : value === null ? [] : [value]
  const chosenOptions = options.filter((option) => chosen.includes(option.value))
  const text = multiple
    ? (valueText?.(chosenOptions) ?? chosenOptions.map((option) => option.label).join(', '))
    : (chosenOptions[0]?.label ?? '')

  const enabled = (index: number) => options[index] !== undefined && !options[index].disabled
  const seek = (index: number, step: 1 | -1) => {
    for (let at = index; at >= 0 && at < options.length; at += step) if (enabled(at)) return at
    return -1
  }
  const show = (at?: number) => {
    const first = options.findIndex((option, index) => chosen.includes(option.value) && enabled(index))
    setActive(at ?? (first >= 0 ? first : seek(0, 1)))
    setOpen(true)
  }
  const close = () => {
    typed.current = { text: '', at: 0 }
    setOpen(false)
  }
  const choose = (index: number) => {
    const picked = options[index]?.value
    if (disabled || picked === undefined || !enabled(index)) return
    if (multiple) {
      const remaining = new Set(chosen)
      if (!remaining.delete(picked)) remaining.add(picked)
      const visible: T[] = []
      for (const option of options) {
        if (remaining.delete(option.value)) visible.push(option.value)
      }
      onChange([...visible, ...remaining])
    } else {
      if (picked !== value) onChange(picked)
      close()
    }
  }

  useDismiss(open, root, trigger, close)
  useInViewport(open, list, trigger)
  useEffect(() => {
    if (open && active >= 0) document.getElementById(optionId(active))?.scrollIntoView?.({ block: 'nearest' })
  }, [open, active, id])

  const typeahead = (buffer: string) => {
    // One repeated letter steps through the options that start with it.
    const start = buffer.length === 1 && open ? active + 1 : 0
    const order = [...options.keys()].slice(start).concat([...options.keys()].slice(0, start))
    const match = order.find((index) => enabled(index) && options[index]?.label.toLowerCase().startsWith(buffer))
    return match
  }

  const keyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.ctrlKey || event.metaKey || event.nativeEvent.isComposing) return
    const { key } = event
    if (key === 'Shift') return
    if (event.altKey) {
      if (key === 'ArrowDown' || key === 'ArrowUp') {
        event.preventDefault()
        if (key === 'ArrowUp') close()
        else if (!open) show()
      }
      return
    }
    if (key === 'Tab') return close()
    const now = Date.now()
    const previous = now - typed.current.at <= 500 ? typed.current.text : ''
    if (key.length === 1 && (key !== ' ' || previous !== '')) {
      const letter = key.toLowerCase()
      const buffer = previous === letter ? letter : previous + letter
      typed.current = { text: buffer, at: now }
      event.preventDefault()
      const match = typeahead(buffer)
      if (match === undefined) return
      return open ? setActive(match) : show(match)
    }
    typed.current = { text: '', at: 0 }
    const moves: Record<string, () => void> = {
      ArrowDown: () => (open ? setActive((at) => (seek(at + 1, 1) >= 0 ? seek(at + 1, 1) : at)) : show()),
      ArrowUp: () => (open ? setActive((at) => (seek(at - 1, -1) >= 0 ? seek(at - 1, -1) : at)) : show()),
      Home: () => (open ? setActive(seek(0, 1)) : show(seek(0, 1))),
      End: () => (open ? setActive(seek(options.length - 1, -1)) : show(seek(options.length - 1, -1))),
      Enter: () => (open ? choose(active) : show()),
      ' ': () => (open ? choose(active) : show())
    }
    if (!Object.hasOwn(moves, key)) return
    // Enter and Space would also click the button, and the arrows would scroll the page.
    event.preventDefault()
    moves[key]?.()
  }

  return (
    <div
      {...rest}
      ref={root}
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-disabled={disabled || undefined}
      data-variant={variant}
    >
      <span className={labelHidden && !help ? styles.hidden : styles.header}>
        <span id={labelId} className={labelHidden ? styles.hidden : styles.label} onClick={() => trigger.current?.focus()}>
          {label}
        </span>
        {help && <Help id={`${id}-bubble`} text={help} label={helpLabel} onActivate={close} />}
      </span>
      {help && (
        <span id={helpId} className={styles.hidden}>
          {help}
        </span>
      )}
      <button
        ref={trigger}
        type="button"
        role="combobox"
        className={styles.control}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-labelledby={labelId}
        aria-describedby={help ? helpId : undefined}
        aria-activedescendant={open && active >= 0 ? optionId(active) : undefined}
        onClick={event => {
          event.currentTarget.focus()
          if (open) close()
          else show()
        }}
        onBlur={close}
        onKeyDown={keyDown}
      >
        <span className={styles.value} data-placeholder={text === '' || undefined}>
          {text === '' ? placeholder : text}
        </span>
        {variant === 'dark' ? (
          <svg className={styles.caret} viewBox="0 0 10 5" aria-hidden="true" focusable="false">
            <path d="M0 0h10L5 5z" />
          </svg>
        ) : (
          <svg className={styles.arrow} viewBox="0 0 12 6" aria-hidden="true" focusable="false">
            <path d="M1 0.75 6 5.25 11 0.75" />
          </svg>
        )}
      </button>
      <div
        ref={list}
        id={listId}
        role="listbox"
        className={styles.list}
        aria-labelledby={labelId}
        aria-multiselectable={multiple || undefined}
        tabIndex={-1}
        hidden={!open}
        // A press on the list must not take the focus from the button.
        onPointerDown={(event) => event.preventDefault()}
        onMouseDown={(event) => event.preventDefault()}
      >
        {options.map((option, index) => (
          <div
            key={option.value}
            id={optionId(index)}
            role="option"
            className={styles.option}
            aria-selected={chosen.includes(option.value)}
            aria-disabled={option.disabled || undefined}
            data-active={index === active || undefined}
            onPointerMove={() => enabled(index) && index !== active && setActive(index)}
            onClick={() => choose(index)}
          >
            <span className={styles.optionLabel}>{option.label}</span>
            <svg className={styles.check} viewBox="0 0 14 10" aria-hidden="true" focusable="false">
              <path d="M1.5 5.2 5 8.5 12.5 1.5" />
            </svg>
          </div>
        ))}
      </div>
      {name !== undefined &&
        chosen.map((entry) => <input key={entry} type="hidden" name={name} value={entry} disabled={disabled} />)}
    </div>
  )
}
