import { useEffect, useState } from 'react'
import { Dropdown } from '../Dropdown/Dropdown'
import styles from './DateField.module.css'

const MONTHS = [
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December'
]

/** The oldest year offered: beyond any living person, and no age policy. */
const FIRST_YEAR = 1900

const pad = (n: number): string => String(n).padStart(2, '0')
const daysInMonth = (year: number, month: number): number =>
  year > 0 && month > 0 ? new Date(Date.UTC(year, month, 0)).getUTCDate() : 31

type Parts = { year: string; month: string; day: string }

const partsOf = (iso: string): Parts => {
  const [, year = '', month = '', day = ''] = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso) ?? []
  return { year, month, day }
}

export type DateFieldProps = {
  /** `YYYY-MM-DD`, or '' for unset. */
  value: string
  /** The composed date, or '' while any part is still unchosen. */
  onChange: (value: string) => void
  disabled?: boolean
  /** `light` is the off-white field of a solid panel. */
  variant?: 'dark' | 'light'
  /** Starts each part's accessible name, for example "Birth Date year". */
  label: string
}

/**
 * A date entered as three HUD selects, day, month and year, instead of `input type="date"`. The browser's
 * date picker is a native popup, which an offscreen host such as the Explorer's CEF HUD cannot composite:
 * nothing here opens one.
 */
export function DateField({ value, onChange, disabled, variant = 'dark', label }: DateFieldProps) {
  const [parts, setParts] = useState<Parts>(() => partsOf(value))

  // Adopt a complete date arriving from outside (a form reseeding), but never replace a part the user has
  // chosen while the rest is still empty: `value` is '' for the whole of that.
  useEffect(() => {
    if (value !== '') setParts(partsOf(value))
  }, [value])

  const dayCount = daysInMonth(Number(parts.year), Number(parts.month))

  const emit = (next: Parts): void => {
    // A day the chosen month does not have (the 31st of February) is pulled back to its last day.
    const limit = daysInMonth(Number(next.year), Number(next.month))
    const day = next.day !== '' && Number(next.day) > limit ? pad(limit) : next.day
    const fixed = { ...next, day }
    setParts(fixed)
    onChange(
      fixed.year !== '' && fixed.month !== '' && fixed.day !== ''
        ? `${fixed.year}-${fixed.month}-${fixed.day}`
        : ''
    )
  }

  const thisYear = new Date().getUTCFullYear()
  const years = Array.from({ length: thisYear - FIRST_YEAR + 1 }, (_, i) => String(thisYear - i))

  return (
    <div className={styles.row}>
      <Dropdown
        appearance="hud"
        label={`${label} day`}
        variant={variant}
        disabled={disabled}
        value={parts.day}
        options={[
          { value: '', label: 'Day' },
          ...Array.from({ length: dayCount }, (_, i) => ({ value: pad(i + 1), label: String(i + 1) }))
        ]}
        onChange={(day) => emit({ ...parts, day })}
      />
      <Dropdown
        appearance="hud"
        label={`${label} month`}
        variant={variant}
        disabled={disabled}
        value={parts.month}
        options={[
          { value: '', label: 'Month' },
          ...MONTHS.map((m, i) => ({ value: pad(i + 1), label: m }))
        ]}
        onChange={(month) => emit({ ...parts, month })}
      />
      <Dropdown
        appearance="hud"
        label={`${label} year`}
        variant={variant}
        disabled={disabled}
        value={parts.year}
        options={[{ value: '', label: 'Year' }, ...years.map((y) => ({ value: y, label: y }))]}
        onChange={(year) => emit({ ...parts, year })}
      />
    </div>
  )
}
