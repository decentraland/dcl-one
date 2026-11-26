import { useId, type CSSProperties, type MouseEvent } from 'react'
import { GeneralLeftArrow, GeneralRightArrow } from '../../../foundations/icons'
import { Tabs } from '../../../components/Tabs/Tabs'
import { TabsBar } from '../../../components/TabsBar/TabsBar'
import { Button } from '../../../components/Button/Button'
import { Busy } from './Busy'
import { DayCard, UpcomingCard } from './cards'
import { MyEvents, type MyEventsProps } from './MyEvents'
import type { ExperienceCardProps } from './types'
import { useEventsCopy } from './locale'
import { Notice } from './Notice'
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
  /** The section's id, so the page can scroll to it. */
  id?: string
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
  /** What My Events shows while it is the active view. */
  mine?: MyEventsProps
  status?: 'ready' | 'loading' | 'unavailable'
  onRetry?: () => void
}

const VIEWS = [
  { id: 'all', label: 'All Events' },
  { id: 'my', label: 'My Events' }
] as const

/**
 * All Events and My Events, source's pill tabs. Each is a link to its URL (so it works without script
 * and can be shared); the page swaps the panel in place.
 */
function Views({
  active,
  hrefs,
  onFollow,
  panelId
}: NonNullable<AllEventsProps['views']> & { panelId: string }) {
  const { text } = useEventsCopy()
  return (
    <TabsBar label={text('Events views')} variant="pill" className={styles.tabs}>
      {VIEWS.map((view) => (
        <Tabs
          key={view.id}
          variant="pill"
          label={text(view.label)}
          href={hrefs[view.id]}
          selected={view.id === active}
          aria-controls={panelId}
          onClick={onFollow && ((event: MouseEvent<HTMLAnchorElement>) => onFollow(view.id, event))}
        />
      ))}
    </TabsBar>
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
  mine,
  status = 'ready',
  onRetry,
  id
}: AllEventsProps) {
  const { text } = useEventsCopy()
  const titleId = useId()
  const panelId = useId()
  const list = days.flatMap((day) => day.cards)
  const panel = views ? { role: 'tabpanel', id: panelId } : {}
  return (
    <section
      id={id}
      className={styles.root}
      aria-labelledby={views ? undefined : titleId}
      aria-label={views ? text('All Events') : undefined}
    >
      {views ? (
        <Views {...views} panelId={panelId} />
      ) : (
        <h2 id={titleId} className={styles.title}>
          {text('All Events')}
        </h2>
      )}
      {views?.active === 'my' && mine ? (
        <div {...panel} aria-label={text('My Events')}>
          <MyEvents {...mine} />
        </div>
      ) : (
        <div
          {...panel}
          aria-label={views ? text('All Events') : undefined}
          className={styles.panel}
        >
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
            <div className={styles.message}>
              <Notice tone="error">
                {text('Events are temporarily unavailable.')}{' '}
                {onRetry ? (
                  <Button variant="tertiary" size="md" onClick={onRetry}>
                    {text('Retry')}
                  </Button>
                ) : null}
              </Notice>
            </div>
          ) : status === 'loading' ? (
            <Busy layout="page">{text('Loading events…')}</Busy>
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
      )}
    </section>
  )
}

export { AllEvents }
export type { AllEventsDay, AllEventsProps, AllEventsTab }
