import { GeneralAdd } from '../../../foundations/icons'
import { Button } from '../../../components/Button/Button'
import { Busy } from './Busy'
import { MyEventCard } from './cards'
import { useEventsCopy } from './locale'
import { Notice } from './Notice'
import type { MyEventCardProps } from './types'
import styles from './MyEvents.module.css'

type MyEventsProps = {
  /** The member's upcoming events, soonest first, including the ones still in review. */
  cards: readonly MyEventCardProps[]
  status: 'ready' | 'loading' | 'unavailable'
  /** Where "Create event" goes when the member has none. */
  createHref: string
  onRetry?: () => void
}

/** My Events: the member's own upcoming events, each opening its editor, or a call to create one. */
function MyEvents({ cards, status, createHref, onRetry }: MyEventsProps) {
  const { text } = useEventsCopy()
  if (status === 'unavailable')
    return (
      <div className={styles.empty}>
        <Notice tone="error">
          {text("Couldn't load your events.")}{' '}
          {onRetry ? (
            <Button variant="tertiary" size="md" onClick={onRetry}>
              {text('Retry')}
            </Button>
          ) : null}
        </Notice>
      </div>
    )
  if (status === 'loading')
    return (
      <div className={styles.empty}>
        <Busy layout="page">{text('Loading your events…')}</Busy>
      </div>
    )
  if (!cards.length)
    return (
      <div role="status" className={styles.empty}>
        <p className={styles.emptyTitle}>{text("You don't have any event created")}</p>
        <Button
          href={createHref}
          variant="tertiary"
          size="md"
          leadingIcon={<GeneralAdd size={16} aria-hidden="true" />}
        >
          {text('Create event')}
        </Button>
      </div>
    )
  return (
    <div className={styles.panel}>
      <h3 className={styles.title}>{text('Hosted by me')}</h3>
      <ul className={styles.grid}>
        {cards.map((card) => (
          <li key={card.id}>
            <MyEventCard {...card} />
          </li>
        ))}
      </ul>
    </div>
  )
}

export { MyEvents }
export type { MyEventsProps }
