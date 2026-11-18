import { type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { ArrowDropDownIcon as ChevronIcon, ExpandMoreIcon as CompactChevronIcon } from '~/components/Icons'
import styles from './Select.module.css'
import { useListbox } from './useListbox'

export type SelectOption<T extends string> = {
  value: T
  label: string
  /** Leading glyph shown before the label, in the trigger and in the list. */
  icon?: ReactNode
  /** Trailing badge (e.g. a rarity's supply), shown in the list and, unless `triggerLabel` is set, in the trigger. */
  trailing?: ReactNode
  /** Compact text for the closed trigger, replacing `label` + `trailing`. */
  triggerLabel?: string
  /** Draws a separator line above this option (the start of a new group). */
  dividerBefore?: boolean
}

type Props<T extends string> = {
  value: T | null
  options: SelectOption<T>[]
  onChange: (value: T) => void
  placeholder?: string
  /** `glyph` is a compact trigger showing only the selected option's icon, for a select sitting inside another field; `compact` is a short uppercase trigger for toolbars. */
  variant?: 'default' | 'glyph' | 'compact'
  /** `dark` sits the select on the editor's dark surfaces instead of the violet ones. */
  tone?: 'default' | 'dark'
  /** A label drawn inside the field, left of the value (the value then aligns right). */
  inlineLabel?: string
  disabled?: boolean
  ariaLabel?: string
  testId?: string
}

const LIST_MIN_WIDTH = { default: 230, glyph: 150, compact: 230 }

/** Custom listbox select: portaled to <body> so scroll containers never clip it, keyboard navigable. */
export function Select<T extends string>({
  value,
  options,
  onChange,
  placeholder,
  variant = 'default',
  tone = 'default',
  inlineLabel,
  disabled = false,
  ariaLabel,
  testId = 'select'
}: Props<T>) {
  const { open, active, setActive, wrapRef, listRef, listStyle, listId, toggle, pick, onKeyDown } = useListbox({
    options,
    current: value,
    onPick: onChange,
    minWidth: LIST_MIN_WIDTH[variant]
  })
  const selected = options.find(option => option.value === value)

  return (
    <div className={styles.wrap} ref={wrapRef} onKeyDown={onKeyDown}>
      <button
        className={styles.trigger}
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-label={ariaLabel ?? inlineLabel}
        disabled={disabled}
        data-testid={testId}
        data-value={value ?? undefined}
        data-variant={variant}
        data-tone={tone}
        onClick={toggle}
      >
        {inlineLabel && <span className={styles.inlineLabel}>{inlineLabel}</span>}
        {variant === 'glyph' ? (
          <span className={styles.optionLabel} title={selected?.label}>
            {selected?.icon}
          </span>
        ) : (
          <span
            className={styles.triggerLabel}
            data-placeholder={selected ? undefined : true}
            data-align={inlineLabel ? 'end' : undefined}
          >
            <span className={styles.optionLabel}>
              {selected?.icon}
              <span>{selected ? (selected.triggerLabel ?? selected.label) : placeholder}</span>
            </span>
            {selected?.trailing && !selected.triggerLabel && (
              <span className={styles.trailing}>{selected.trailing}</span>
            )}
          </span>
        )}
        {variant === 'compact' ? <CompactChevronIcon /> : <ChevronIcon />}
      </button>
      {open &&
        createPortal(
          <ul
            className={styles.listbox}
            ref={listRef}
            role="listbox"
            id={listId}
            data-testid={`${testId}-listbox`}
            data-tone={tone}
            style={listStyle}
            onKeyDown={onKeyDown}
          >
            {options.map(option => (
              <li className={styles.optionGroup} key={option.value} data-divider={option.dividerBefore || undefined}>
                <div
                  className={styles.option}
                  role="option"
                  aria-selected={option.value === value}
                  data-active={option.value === active || undefined}
                  data-testid={`${testId}-option-${option.value}`}
                  onMouseEnter={() => setActive(option.value)}
                  onMouseLeave={() => setActive(value)}
                  onClick={() => pick(option.value)}
                >
                  <span className={styles.optionLabel}>
                    {option.icon}
                    <span>{option.label}</span>
                  </span>
                  {option.trailing && <span className={styles.trailing}>{option.trailing}</span>}
                </div>
              </li>
            ))}
          </ul>,
          document.body
        )}
    </div>
  )
}
