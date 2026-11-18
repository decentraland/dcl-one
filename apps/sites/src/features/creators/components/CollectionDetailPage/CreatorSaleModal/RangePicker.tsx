import { useCallback, useEffect, useImperativeHandle, useRef, useState, type Ref } from 'react'
import DatePicker from 'react-datepicker'
import { enUS } from 'date-fns/locale'
import 'react-datepicker/dist/react-datepicker.css'
import { useMorphFrom } from '../../../hooks/useMorphFrom'
import { useIntl } from '@/data/lib/creators/intl'
import { useTranslation } from '@/data/lib/creators/intl'
import { breakpoints } from '@/data/lib/creators/theme'
import styles from './RangePicker.module.css'

// The calendar's weekday and month names in the viewer's language.

// One px past `sm`, so the two-month calendar and the phone's stacked presets never both apply.
const WIDE = `(min-width: ${breakpoints.sm + 1}px)`

/** Before the first sale any store can have, so a stray click decades back cannot ask the chart for thousands of points. */
const EARLIEST = new Date(2020, 0, 1)

function useWide(): boolean {
  const [wide, setWide] = useState(() => typeof window !== 'undefined' && window.matchMedia?.(WIDE).matches)
  useEffect(() => {
    const list = window.matchMedia?.(WIDE)
    if (!list) return
    const onChange = () => setWide(list.matches)
    list.addEventListener('change', onChange)
    return () => list.removeEventListener('change', onChange)
  }, [])
  return !!wide
}

/** With two months shown, the one holding the range goes on the right, so the month before it is the left one. */
function openOn(date: Date, twoMonths: boolean): Date {
  if (!twoMonths) return date
  return new Date(date.getFullYear(), date.getMonth() - 1, 1)
}

/** Lets the trigger fold the picker away through the same animation as every other way out. */
export type RangePickerHandle = { close: () => void }

/**
 * A two-click range picker in the Shop's calendar, opened from a trigger it sits under.
 *
 * Nothing changes until Apply: picking the first day of a range is half a choice, and re-reading the whole
 * dashboard on it would redraw every figure for a range nobody asked for.
 */
export function RangePicker({
  from,
  to,
  min = EARLIEST.getTime(),
  max,
  presets,
  handle,
  stretch = false,
  label: labelProp,
  triggerSelector = '[data-range-trigger]',
  testId = 'store-range',
  presetTestId = 'store-period',
  onApply,
  onClose
}: {
  from: number | undefined
  to: number | undefined
  /** The first and last days that can be picked; with no `max` the calendar runs into the future. */
  min?: number
  /** Spans its anchor's width with a single month, for a trigger too narrow for two months beside the presets. */
  stretch?: boolean
  label?: string
  /** The one element that opens this picker, which it grows out of and hands focus back to. */
  triggerSelector?: string
  testId?: string
  presetTestId?: string
  /** Ready-made ranges listed beside the calendar; picking one applies it straight away. */
  presets?: { key: string; label: string; active: boolean; onPick: () => void }[]
  handle?: Ref<RangePickerHandle>
  onApply: (from: number, to: number) => void
  onClose: () => void
  /** The last day that can be picked; without it the calendar runs into the future. */
  max?: number
}) {
  const { t } = useTranslation()
  const { locale } = useIntl()
  const label = labelProp ?? t('creator_sale.range_period')
  const [start, setStart] = useState<Date | null>(from != null ? new Date(from) : null)
  const [end, setEnd] = useState<Date | null>(to != null ? new Date(to) : null)
  const ref = useRef<HTMLDivElement>(null)
  const wide = useWide()
  const twoMonths = wide && !stretch
  const leave = useMorphFrom(ref, triggerSelector)
  const close = useCallback(() => leave(onClose), [leave, onClose])
  useImperativeHandle(handle, () => ({ close }), [close])

  useEffect(() => {
    function onPointer(event: PointerEvent) {
      const target = event.target as Node
      if (ref.current?.contains(target)) return
      // The trigger closes the picker from its own click; closing here too would let that click reopen it.
      if ((target as Element).closest?.(triggerSelector)) return
      close()
    }
    function onKey(event: KeyboardEvent) {
      if (event.key === 'Escape') close()
      if (event.key !== 'Tab' || !ref.current) return
      // Keeps Tab inside the dialog while it is open, wrapping at either end.
      const focusable = [
        ...ref.current.querySelectorAll<HTMLElement>('button:not([disabled]), select:not([disabled]), [tabindex="0"]')
      ]
      if (focusable.length === 0) return
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('pointerdown', onPointer)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('pointerdown', onPointer)
      document.removeEventListener('keydown', onKey)
    }
  }, [close, triggerSelector])

  // Focus goes in on open and back to the trigger on the way out, so a keyboard user is never left behind.
  useEffect(() => {
    const panel = ref.current
    const trigger = document.activeElement as HTMLElement | null
    // The presets come first and are the usual pick, so focus starts on the one in force.
    const preset = panel?.querySelector<HTMLElement>('[data-preset][aria-pressed="true"]')
    const day = panel?.querySelector<HTMLElement>('.react-datepicker__day[tabindex="0"]')
    ;(preset ?? day ?? panel)?.focus()
    return () => {
      // The panel unmounts after its fold, by when a press elsewhere may have focused what it pressed.
      const active = document.activeElement
      if (active && active !== document.body && !panel?.contains(active)) return
      // Safari does not focus a button on click, so the element focused at open may not be the trigger.
      const fallback = document.querySelector<HTMLElement>(triggerSelector)
      ;(trigger?.isConnected && trigger.matches(triggerSelector) ? trigger : fallback)?.focus()
    }
  }, [triggerSelector])

  const format = new Intl.DateTimeFormat(locale, { month: 'short', day: 'numeric', year: 'numeric' })
  const summary = start
    ? end
      ? t('creator_sale.when_range', { from: format.format(start), to: format.format(end) })
      : t('creator_sale.range_pick_end', { from: format.format(start) })
    : t('creator_sale.range_pick_start')

  return (
    <div
      className={styles.root}
      ref={ref}
      role="dialog"
      aria-modal="true"
      aria-label={label}
      tabIndex={-1}
      data-stretch={stretch ? '' : undefined}
      data-testid={`${testId}-picker`}
    >
      <div className={styles.body}>
        {presets?.length ? (
          <div className={styles.presets} role="group" aria-label={t('creator_sale.range_presets')}>
            {presets.map(preset => (
              <button
                className={styles.preset}
                key={preset.key}
                type="button"
                aria-pressed={preset.active}
                data-preset=""
                onClick={() => leave(preset.onPick)}
                data-testid={`${presetTestId}-${preset.key}`}
              >
                {preset.label}
              </button>
            ))}
          </div>
        ) : null}
        <DatePicker
          inline
          selectsRange
          startDate={start}
          endDate={end}
          onChange={([nextStart, nextEnd]) => {
            setStart(nextStart)
            setEnd(nextEnd)
          }}
          maxDate={max != null ? new Date(max) : undefined}
          minDate={new Date(min)}
          monthsShown={twoMonths ? 2 : 1}
          openToDate={openOn(end ?? start ?? new Date(max ?? Date.now()), twoMonths)}
          calendarStartDay={1}
          locale={enUS}
        />
      </div>
      <div className={styles.foot}>
        <span data-testid={`${testId}-summary`}>{summary}</span>
        <div className={styles.actions}>
          <button className={styles.btn} type="button" onClick={close}>
            {t('creator_sale.range_cancel')}
          </button>
          <button
            className={styles.btn}
            type="button"
            data-variant="primary"
            disabled={!start || !end}
            onClick={() => start && end && leave(() => onApply(start.getTime(), end.getTime()))}
            data-testid={`${testId}-apply`}
          >
            {t('creator_sale.range_apply')}
          </button>
        </div>
      </div>
    </div>
  )
}
