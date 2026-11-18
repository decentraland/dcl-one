import { useMemo } from 'react'
import { createPortal } from 'react-dom'
import { ArrowDropDownIcon as ChevronIcon, CheckIcon, ExpandMoreIcon as CompactChevronIcon } from '@/features/creators/components/Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import base from '@/features/creators/components/Select/Select.module.css'
import { useListbox } from '@/features/creators/components/Select/useListbox'
import styles from './MultiSelect.module.css'

export type MultiSelectOption<T extends string> = { value: T; label: string }

type Props<T extends string> = {
  values: T[]
  options: MultiSelectOption<T>[]
  onChange: (values: T[]) => void
  placeholder?: string
  /** Trigger text once something is picked, instead of the comma list of labels. */
  summary?: string
  disabled?: boolean
  tone?: 'default' | 'dark'
  /** `compact` is the short uppercase toolbar trigger, as in `Select`. */
  variant?: 'default' | 'compact'
  ariaLabel?: string
  testId?: string
}

const LIST_MIN_WIDTH = 230

/** Multiple-choice listbox: the trigger lists the picked labels, options toggle without closing the list. */
export function MultiSelect<T extends string>({
  values,
  options,
  onChange,
  placeholder,
  summary: summaryOverride,
  disabled = false,
  tone = 'default',
  variant = 'default',
  ariaLabel,
  testId = 'multi-select'
}: Props<T>) {
  const { t } = useTranslation()
  const { open, active, setActive, wrapRef, listRef, listStyle, listId, toggle, pick, onKeyDown } = useListbox({
    options,
    current: values[0] ?? null,
    onPick: value =>
      onChange(values.includes(value) ? values.filter(candidate => candidate !== value) : [...values, value]),
    keepOpen: true,
    minWidth: LIST_MIN_WIDTH
  })
  const selected = useMemo(() => options.filter(option => values.includes(option.value)), [options, values])
  // The short compact trigger counts the picks beyond the first instead of truncating a comma list.
  const summary =
    selected.length === 0
      ? placeholder
      : summaryOverride
        ? summaryOverride
        : variant === 'compact' && selected.length > 1
          ? t('multi_select.summary', { first: selected[0].label, count: selected.length - 1 })
          : selected.map(option => option.label).join(', ')

  return (
    <div className={base.wrap} ref={wrapRef} onKeyDown={onKeyDown}>
      <button
        className={base.trigger}
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-label={ariaLabel && summary ? `${ariaLabel}: ${summary}` : ariaLabel}
        disabled={disabled}
        data-testid={testId}
        data-tone={tone}
        data-variant={variant}
        data-count={values.length}
        onClick={toggle}
      >
        <span className={base.triggerLabel} data-placeholder={selected.length === 0 ? true : undefined}>
          <span className={styles.summary}>{summary}</span>
        </span>
        {variant === 'compact' ? <CompactChevronIcon /> : <ChevronIcon />}
      </button>
      {open &&
        createPortal(
          <ul
            className={base.listbox}
            ref={listRef}
            role="listbox"
            aria-multiselectable="true"
            id={listId}
            data-testid={`${testId}-listbox`}
            data-tone={tone}
            style={listStyle}
            onKeyDown={onKeyDown}
          >
            {options.map(option => {
              const checked = values.includes(option.value)
              return (
                <li className={base.optionGroup} key={option.value}>
                  <div
                    className={base.option}
                    role="option"
                    aria-selected={checked}
                    data-active={option.value === active || undefined}
                    data-testid={`${testId}-option-${option.value}`}
                    onMouseEnter={() => setActive(option.value)}
                    onClick={() => pick(option.value)}
                  >
                    <span className={base.optionLabel}>
                      <span className={styles.check} aria-hidden data-checked={checked || undefined}>
                        {checked && <CheckIcon />}
                      </span>
                      <span>{option.label}</span>
                    </span>
                  </div>
                </li>
              )
            })}
          </ul>,
          document.body
        )}
    </div>
  )
}
