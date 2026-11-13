import { Button } from '../shared/Button/Button'
import { CalendarAddIcon, REMIND_BELL, ShareIcon } from './JumpIcons'
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

/** The bell outline, or the filled bell with its check once the reminder is set (sites: RemindMeIcon). */
function RemindBell({ active }: { active: boolean }) {
  return (
    <span className={styles.bell} aria-hidden="true">
      <svg
        className={active ? `${styles.bellLayer} ${styles.hidden}` : styles.bellLayer}
        viewBox="0 0 25 25"
        fill="currentColor"
      >
        <path d={REMIND_BELL.body} />
        <path d={REMIND_BELL.clapper} />
      </svg>
      <svg
        className={active ? styles.bellLayer : `${styles.bellLayer} ${styles.hidden}`}
        viewBox="0 0 25 25"
        fill="currentColor"
      >
        <path d={REMIND_BELL.active} />
        <path d={REMIND_BELL.check} />
      </svg>
    </span>
  )
}

/** What an event card offers instead of Jump In; a live event keeps Jump In and renders none of this. */
function JumpEventActions(props: JumpEventActionsProps) {
  if (props.state === 'deleted') {
    return (
      <div className={styles.root}>
        <p className={styles.notice}>
          This event is no longer available. Browse <a href={props.eventsHref}>All Events</a> to
          discover more.
        </p>
      </div>
    )
  }
  if (props.state === 'ended') {
    return (
      <div className={styles.root}>
        <Button href={props.eventsHref} variant="secondary" className={styles.explore}>
          EXPLORE EVENTS
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
          className={styles.calendar}
        >
          <CalendarAddIcon />
          ADD TO CALENDAR
        </Button>
      ) : null}
      <Button
        variant="tertiary"
        aria-pressed={remind.active}
        disabled={remind.pending}
        onClick={remind.onToggle}
        className={styles.remind}
      >
        <RemindBell active={remind.active} />
        <span className={styles.label}>REMIND ME</span>
      </Button>
      <Button variant="tertiary" aria-label="Share" onClick={onShare} className={styles.share}>
        <ShareIcon />
      </Button>
      <span className={styles.status} role="status">
        {shareNotice}
      </span>
    </div>
  )
}

export { JumpEventActions }
export type { JumpEventActionsProps }
