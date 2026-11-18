import type { KeyboardEvent, ReactNode } from 'react'
import { FemaleIcon, MaleIcon, TransgenderIcon as BothIcon } from '../Icons'
import { Tooltip } from '../Tooltip'
import { useTranslation } from '@/data/lib/creators/intl'
import { BodyShapeType } from '@/data/lib/creators/items'
import styles from './ItemDraftModal.module.css'

const OPTIONS: Array<{ value: BodyShapeType; icon: ReactNode }> = [
  { value: BodyShapeType.BOTH, icon: <BothIcon /> },
  { value: BodyShapeType.FEMALE, icon: <FemaleIcon /> },
  { value: BodyShapeType.MALE, icon: <MaleIcon /> }
]

type Props = {
  value: BodyShapeType
  /** The pick is fixed (unisex kinds, a representation being added): the other options only show the hint. */
  locked?: boolean
  lockedHint?: string | null
  onChange: (value: BodyShapeType) => void
  testIdPrefix?: string
}

/** Both / Female / Male segmented control, an ARIA radio group: one tab stop, arrow keys move the pick. */
export function BodyShapePicker({
  value,
  locked = false,
  lockedHint = null,
  onChange,
  testIdPrefix = 'body-shape'
}: Props) {
  const { t } = useTranslation()

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (locked) return
    const index = OPTIONS.findIndex(option => option.value === value)
    const steps: Record<string, number> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }
    const next =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? OPTIONS.length - 1
          : steps[event.key] === undefined
            ? null
            : (index + steps[event.key] + OPTIONS.length) % OPTIONS.length
    if (next === null) return
    event.preventDefault()
    onChange(OPTIONS[next].value)
    event.currentTarget.querySelector<HTMLElement>(`[data-testid="${testIdPrefix}-${OPTIONS[next].value}"]`)?.focus()
  }

  return (
    <div
      className={styles.segmented}
      role="radiogroup"
      aria-label={t('add_items_modal.body_shape')}
      onKeyDown={onKeyDown}
    >
      {OPTIONS.map(({ value: option, icon }) => {
        const selected = option === value
        return (
          <Tooltip
            key={option}
            content={locked && !selected ? lockedHint : null}
            asChild
            testId={`${testIdPrefix}-${option}-tooltip`}
          >
            <button
              className={styles.segmentButton}
              type="button"
              role="radio"
              aria-disabled={locked || undefined}
              aria-checked={selected}
              data-selected={selected || undefined}
              data-testid={`${testIdPrefix}-${option}`}
              tabIndex={selected ? 0 : -1}
              onClick={() => {
                if (!locked) onChange(option)
              }}
            >
              {icon}
              {t(`add_items_modal.body_shape_option.${option}`)}
            </button>
          </Tooltip>
        )
      })}
    </div>
  )
}
