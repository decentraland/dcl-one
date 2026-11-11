import { useId, useRef, type CSSProperties, type KeyboardEvent } from 'react'
import { GeneralLeftArrow, GeneralRightArrow } from '../../../foundations/icons'
import { Button } from '../shared/Button/Button'
import { ExperienceCardSlot } from './cards/ExperienceCardSlot'
import { UpcomingEventCardSlot } from './cards/UpcomingEventCardSlot'
import type { ExperienceCardProps } from './cards/types'
import styles from './AllEvents.module.css'

type AllEventsDay = {
  /** Stable per day, e.g. its ISO date. */
  key: string
  /** "Today", "Tomorrow", "Fri, Nov 13". */
  label: string
  /** The full date for assistive technology. */
  fullLabel: string
  isToday: boolean
  cards: readonly ExperienceCardProps[]
}

type AllEventsTab = 'all' | 'my'

type AllEventsProps = {
  days: readonly AllEventsDay[]
  /** One column per day from tablet up; one list on phones, as source. */
  layout: 'columns' | 'list'
  canGoBack: boolean
  onPrevious: () => void
  onNext: () => void
  /** Shown to signed-in members only, as source; omitted, the section shows its title. */
  tabs?: { active: AllEventsTab; onSelect: (tab: AllEventsTab) => void }
  status?: 'ready' | 'loading' | 'unavailable'
  onRetry?: () => void
}

const TABS: readonly { id: AllEventsTab; label: string }[] = [
  { id: 'all', label: 'All Events' },
  { id: 'my', label: 'My Events' }
]

// TODO(figma-ds): a minimal accessible tablist until the design system ships Tabs.
function Tabs({
  active,
  onSelect,
  panelId
}: NonNullable<AllEventsProps['tabs']> & { panelId: string }) {
  const list = useRef<HTMLDivElement | null>(null)
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return
    event.preventDefault()
    const index = TABS.findIndex((tab) => tab.id === active)
    const next = TABS[(index + (event.key === 'ArrowRight' ? 1 : -1) + TABS.length) % TABS.length]
    if (!next) return
    onSelect(next.id)
    list.current?.querySelector<HTMLButtonElement>(`[data-tab="${next.id}"]`)?.focus()
  }
  return (
    <div ref={list} role="tablist" aria-label="All Events" className={styles.tabs}>
      {TABS.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          data-tab={tab.id}
          aria-selected={tab.id === active}
          aria-controls={panelId}
          tabIndex={tab.id === active ? 0 : -1}
          className={styles.tab}
          onClick={() => onSelect(tab.id)}
          onKeyDown={onKeyDown}
        >
          {tab.label}
        </button>
      ))}
    </div>
  )
}

function DayColumn({ day }: { day: AllEventsDay }) {
  return (
    <ul className={styles.column} aria-label={`Events for ${day.fullLabel}`}>
      {day.cards.length ? (
        day.cards.map((card) => (
          <li key={card.id}>
            <ExperienceCardSlot {...card} />
          </li>
        ))
      ) : (
        <li className={styles.empty}>No events this day.</li>
      )}
    </ul>
  )
}

/** Every event, day by day, with previous and next days. */
function AllEvents({
  days,
  layout,
  canGoBack,
  onPrevious,
  onNext,
  tabs,
  status = 'ready',
  onRetry
}: AllEventsProps) {
  const titleId = useId()
  const panelId = useId()
  const list = days.flatMap((day) => day.cards)
  return (
    <section
      className={styles.root}
      aria-labelledby={tabs ? undefined : titleId}
      aria-label={tabs ? 'All Events' : undefined}
    >
      {tabs ? (
        <Tabs {...tabs} panelId={panelId} />
      ) : (
        <h2 id={titleId} className={styles.title}>
          All Events
        </h2>
      )}
      <div id={panelId} role={tabs ? 'tabpanel' : undefined} className={styles.panel}>
        <nav className={styles.dates} aria-label="Event days">
          <button
            type="button"
            className={styles.dateArrow}
            aria-label="Navigate to previous dates"
            disabled={!canGoBack}
            onClick={onPrevious}
          >
            <GeneralLeftArrow size={20} aria-hidden="true" />
          </button>
          <ol className={styles.dateLabels}>
            {days.map((day) => (
              <li
                key={day.key}
                className={styles.dateLabel}
                aria-current={day.isToday ? 'date' : undefined}
              >
                <span aria-hidden="true">{day.label}</span>
                <span className={styles.visuallyHidden}>{day.fullLabel}</span>
              </li>
            ))}
          </ol>
          <button
            type="button"
            className={styles.dateArrow}
            aria-label="Navigate to next dates"
            onClick={onNext}
          >
            <GeneralRightArrow size={20} aria-hidden="true" />
          </button>
        </nav>
        {status === 'unavailable' ? (
          <div role="alert" className={styles.message}>
            <p>Events are temporarily unavailable.</p>
            {onRetry ? (
              <Button variant="tertiary" size="sm" onClick={onRetry}>
                Retry
              </Button>
            ) : null}
          </div>
        ) : status === 'loading' ? (
          <p className={styles.message}>Loading events…</p>
        ) : layout === 'list' ? (
          list.length ? (
            <ul
              className={styles.list}
              aria-label={days[0] ? `Events for ${days[0].fullLabel}` : undefined}
            >
              {list.map((card) => (
                <li key={card.id}>
                  <UpcomingEventCardSlot {...card} />
                </li>
              ))}
            </ul>
          ) : (
            <p className={styles.message}>No events this day.</p>
          )
        ) : (
          <div className={styles.columns} style={{ '--columns': days.length } as CSSProperties}>
            {days.map((day) => (
              <DayColumn key={day.key} day={day} />
            ))}
          </div>
        )}
      </div>
    </section>
  )
}

export { AllEvents }
export type { AllEventsDay, AllEventsProps, AllEventsTab }
