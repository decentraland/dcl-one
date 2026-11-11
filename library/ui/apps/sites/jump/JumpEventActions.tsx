import { GeneralAddToCalendar, GeneralBell, MenusShare } from '../../../foundations/icons'
import { Button } from '../shared/Button/Button'
import styles from './JumpEventActions.module.css'

type JumpEventActionsProps =
  | { state: 'deleted'; eventsHref: string }
  | { state: 'ended'; eventsHref: string }
  | {
      state: 'upcoming'
      calendarHref?: string
      remind: { active: boolean; pending: boolean; onToggle: () => void }
      onShare: () => void
      /** Set after a share copied the link instead of opening the share sheet. */
      shareNotice?: string
    }

/** What an event card offers instead of Jump In; a live event keeps Jump In and renders none of this. */
function JumpEventActions(props: JumpEventActionsProps) {
  if (props.state === 'deleted') {
    return (
      <p className={styles.notice} role="status">
        This event is no longer available. Browse <a href={props.eventsHref}>All Events</a> to
        discover more.
      </p>
    )
  }
  if (props.state === 'ended') {
    return (
      <div className={styles.root}>
        <Button href={props.eventsHref} variant="secondary" className={styles.wide}>
          Explore events
        </Button>
      </div>
    )
  }
  const { calendarHref, remind, onShare, shareNotice } = props
  return (
    <div className={styles.root}>
      {calendarHref ? (
        <Button
          href={calendarHref}
          target="_blank"
          variant="tertiary"
          aria-label="Add to calendar"
          className={styles.grow}
        >
          <GeneralAddToCalendar size={16} aria-hidden="true" />
          Add to calendar
        </Button>
      ) : null}
      <Button
        variant={remind.active ? 'primary' : 'tertiary'}
        aria-pressed={remind.active}
        disabled={remind.pending}
        onClick={remind.onToggle}
        className={styles.grow}
      >
        <GeneralBell size={16} aria-hidden="true" />
        Remind me
      </Button>
      <Button variant="iconOnly" aria-label="Share" onClick={onShare}>
        <MenusShare size={16} aria-hidden="true" />
      </Button>
      <span className={styles.status} role="status">
        {shareNotice}
      </span>
    </div>
  )
}

export { JumpEventActions }
export type { JumpEventActionsProps }
