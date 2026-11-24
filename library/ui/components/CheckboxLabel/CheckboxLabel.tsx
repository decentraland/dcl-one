import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { GeneralHelp } from '../../foundations/icons/general'
import { CheckboxControl, type CheckboxControlProps } from '../Checkbox/Checkbox'
import styles from './CheckboxLabel.module.css'

export type CheckboxLabelPosition = 'end' | 'start'

export type CheckboxLabelProps = Omit<CheckboxControlProps, 'aria-label' | 'aria-labelledby'> & {
  /** The visible label, which is also the checkbox's accessible name. */
  label: ReactNode
  /** `end` (default): box, then text. `start`: text, then box. */
  labelPosition?: CheckboxLabelPosition
  /**
   * Shows the help button after the text. The text is the checkbox's description and opens
   * below the button on click, tap, Enter or Space.
   */
  help?: string
  /** The help button's name. Defaults to "More information". */
  helpLabel?: string
}

/**
 * A checkbox with visible text. The text is the native label, so clicking it toggles the box. The
 * help button sits outside the label: it never toggles the box and never joins its name.
 */
export function CheckboxLabel({
  label,
  labelPosition = 'end',
  help,
  helpLabel = 'More information',
  className,
  id,
  'aria-describedby': describedBy,
  ...control
}: CheckboxLabelProps) {
  const generatedId = useId()
  const inputId = id ?? `${generatedId}-input`
  const helpId = `${generatedId}-help`
  const description = [describedBy, help ? helpId : undefined].filter(Boolean).join(' ')
  const box = (
    <CheckboxControl {...control} id={inputId} aria-describedby={description || undefined} />
  )
  return (
    <span
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-position={labelPosition}
      data-disabled={control.disabled || undefined}
    >
      {labelPosition === 'end' && box}
      <span className={styles.text}>
        <label className={styles.label} htmlFor={inputId}>
          {label}
        </label>
        {help && <HelpDisclosure id={`${generatedId}-bubble`} text={help} label={helpLabel} />}
        {help && (
          <span id={helpId} className={styles.hidden}>
            {help}
          </span>
        )}
      </span>
      {labelPosition === 'start' && box}
    </span>
  )
}

/** A help button that shows its text until it is pressed again, Escape is pressed or focus leaves. */
function HelpDisclosure({ id, text, label }: { id: string; text: string; label: string }) {
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLSpanElement>(null)
  const button = useRef<HTMLButtonElement>(null)
  const bubble = useRef<HTMLSpanElement>(null)
  // Centred below the button; above it when it does not fit below and fits better above; then
  // moved sideways back inside the viewport.
  useLayoutEffect(() => {
    const element = bubble.current
    if (!open || !element) return
    const gutter = 8
    const viewport = document.documentElement
    element.style.setProperty('--help-shift', '0px')
    delete element.dataset.side
    const below = element.getBoundingClientRect()
    const anchor = button.current!.getBoundingClientRect()
    const roomBelow = viewport.clientHeight - gutter - anchor.bottom
    const roomAbove = anchor.top - gutter
    if (below.bottom > viewport.clientHeight - gutter && roomAbove > roomBelow)
      element.dataset.side = 'top'
    const { left, right } = element.getBoundingClientRect()
    const room = viewport.clientWidth - gutter
    const shift = left < gutter ? gutter - left : right > room ? room - right : 0
    element.style.setProperty('--help-shift', `${shift}px`)
  }, [open])
  useEffect(() => {
    if (!open) return
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false)
    }
    // Escape closes the help only: consumed before an enclosing dialog or page handler sees it.
    const escape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      setOpen(false)
      button.current?.focus()
    }
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', escape, true)
    return () => {
      document.removeEventListener('pointerdown', outside)
      document.removeEventListener('keydown', escape, true)
    }
  }, [open])
  return (
    <span
      ref={root}
      className={styles.help}
      onBlur={(event) => {
        if (!root.current?.contains(event.relatedTarget as Node | null)) setOpen(false)
      }}
    >
      <button
        ref={button}
        type="button"
        className={styles.helpButton}
        aria-label={label}
        aria-expanded={open}
        aria-controls={id}
        onClick={() => setOpen((value) => !value)}
      >
        <GeneralHelp size={18} aria-hidden="true" focusable="false" />
      </button>
      {/* Focusable by pointer only, so tapping the open text keeps focus inside and it open. */}
      <span
        ref={bubble}
        id={id}
        className={styles.bubble}
        hidden={!open}
        tabIndex={-1}
      >
        {text}
      </span>
    </span>
  )
}
