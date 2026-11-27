import { Button } from '../../../components/Button/Button'
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

/** The bell outline, or the filled bell with its check once the reminder is set. */
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
        <Button href={props.eventsHref} variant="secondary" size="md" className={styles.explore}>
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
          variant="outline"
          size="md"
          leadingIcon={<CalendarAddIcon />}
        >
          Add to calendar
        </Button>
      ) : null}
      {/* As the events dialog's Remind me: secondaryLight until set, then primary. */}
      <Button
        variant={remind.active ? 'primary' : 'secondaryLight'}
        size="md"
        aria-pressed={remind.active}
        disabled={remind.pending}
        onClick={remind.onToggle}
        leadingIcon={<RemindBell active={remind.active} />}
      >
        Remind me
      </Button>
      <Button
        variant="outline"
        size="md"
        iconOnly
        aria-label="Share"
        onClick={onShare}
        className={styles.share}
      >
        <ShareIcon className={styles.shareIcon} />
      </Button>
      <span className={styles.status} role="status">
        {shareNotice}
      </span>
    </div>
  )
}

export { JumpEventActions }
export type { JumpEventActionsProps }
