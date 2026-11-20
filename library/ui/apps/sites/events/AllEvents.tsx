import { useId, type CSSProperties, type MouseEvent } from 'react'
import { GeneralLeftArrow, GeneralRightArrow } from '../../../foundations/icons'
import { Button } from '../shared/Button/Button'
import { DayCard, UpcomingCard } from './cards'
import type { ExperienceCardProps } from './types'
import { useEventsCopy } from './locale'
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
  /** False at the last day the page offers. */
  canGoForward?: boolean
  onPrevious: () => void
  onNext: () => void
  /** All Events and My Events, for signed-in members only, as source; omitted, the section shows its title. */
  views?: {
    active: AllEventsTab
    hrefs: Record<AllEventsTab, string>
    /** Runs before a link is followed, so the page can navigate in place. */
    onFollow?: (view: AllEventsTab, event: MouseEvent<HTMLAnchorElement>) => void
  }
  status?: 'ready' | 'loading' | 'unavailable'
  onRetry?: () => void
}

const VIEWS = [
  { id: 'all', label: 'All Events' },
  { id: 'my', label: 'My Events' }
] as const

/** All Events and My Events are two pages, so they are links, the current one marked. */
function Views({ active, hrefs, onFollow }: NonNullable<AllEventsProps['views']>) {
  const { text } = useEventsCopy()
  return (
    <nav aria-label={text('Events views')} className={styles.tabs}>
      {VIEWS.map((view) => (
        <a
          key={view.id}
          href={hrefs[view.id]}
          aria-current={view.id === active ? 'page' : undefined}
          className={styles.tab}
          onClick={onFollow && ((event) => onFollow(view.id, event))}
        >
          {text(view.label)}
        </a>
      ))}
    </nav>
  )
}

function DayColumn({ day }: { day: AllEventsDay }) {
  const { text } = useEventsCopy()
  return (
    <ul className={styles.column} aria-label={text('Events for {day}', { day: day.fullLabel })}>
      {day.cards.length ? (
        day.cards.map((card) => (
          <li key={card.id}>
            <DayCard {...card} />
          </li>
        ))
      ) : (
        <li className={styles.empty}>{text('No events this day.')}</li>
      )}
      <li className={styles.filler} aria-hidden="true" />
    </ul>
  )
}

/** Every event, day by day, with previous and next days. */
function AllEvents({
  days,
  layout,
  canGoBack,
  canGoForward = true,
  onPrevious,
  onNext,
  views,
  status = 'ready',
  onRetry
}: AllEventsProps) {
  const { text } = useEventsCopy()
  const titleId = useId()
  const list = days.flatMap((day) => day.cards)
  return (
    <section
      className={styles.root}
      aria-labelledby={views ? undefined : titleId}
      aria-label={views ? text('All Events') : undefined}
    >
      {views ? (
        <Views {...views} />
      ) : (
        <h2 id={titleId} className={styles.title}>
          {text('All Events')}
        </h2>
      )}
      <div className={styles.panel}>
        <nav className={styles.dates} aria-label={text('Event days')}>
          <button
            type="button"
            className={styles.dateArrow}
            aria-label={text('Navigate to previous dates')}
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
            aria-label={text('Navigate to next dates')}
            disabled={!canGoForward}
            onClick={onNext}
          >
            <GeneralRightArrow size={20} aria-hidden="true" />
          </button>
        </nav>
        {status === 'unavailable' ? (
          <div role="alert" className={styles.message}>
            <p>{text('Events are temporarily unavailable.')}</p>
            {onRetry ? (
              <Button variant="tertiary" size="sm" onClick={onRetry}>
                {text('Retry')}
              </Button>
            ) : null}
          </div>
        ) : status === 'loading' ? (
          <p className={styles.message}>{text('Loading events…')}</p>
        ) : layout === 'list' ? (
          list.length ? (
            <ul
              className={styles.list}
              aria-label={
                days[0] ? text('Events for {day}', { day: days[0].fullLabel }) : undefined
              }
            >
              {list.map((card) => (
                <li key={card.id}>
                  <UpcomingCard {...card} />
                </li>
              ))}
            </ul>
          ) : (
            <p className={styles.message}>{text('No events this day.')}</p>
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
