// An event in the map's panel: when it runs, who hosts it and where, and its time table.

import { useRef, useState } from 'react'
import { setEventAttending, type MapEvent } from '@explorer-core/features/map/mapApi'
import type { EventView } from '@explorer-core/features/map/navmap'
import { calendarLink, eventSchedule, eventShareLinks, upcomingDates } from '@explorer-core/features/map/shareLinks'
import { MaskIcon } from '../../../design/MaskIcon'
import { PANEL_ART } from '../mapArt'
import { ShareMenu } from './ShareMenu'
import { TextWithLinks } from './TextWithLinks'
import styles from './MapPanel.module.css'

export function EventInfo({
  view,
  referrer,
  onEventChange,
  onJumpIn
}: {
  view: EventView
  referrer: string | undefined
  onEventChange: (event: MapEvent) => void
  onJumpIn: () => void
}): React.JSX.Element {
  const { event, place } = view
  const [sharing, setSharing] = useState(false)
  const current = useRef(event)
  current.current = event
  const live = event.live === true
  const attending = event.attending === true

  const remind = (): void => {
    const before = current.current
    const next = before.attending !== true
    const count = (before.total_attendees ?? 0) + (next ? 1 : -1)
    onEventChange({ ...before, attending: next, total_attendees: Math.max(0, count) })
    setEventAttending(before.id, next).catch(() => onEventChange({ ...current.current, attending: before.attending, total_attendees: before.total_attendees }))
  }

  return (
    <div className={styles.info}>
      <div className={styles.thumbnail}>{event.image != null && event.image !== '' && <img src={event.image} alt="" />}</div>
      <div className={styles.eventScroll}>
        {live ? (
          <div className={styles.eventLive}>
            <span className={styles.eventLiveBadge}>
              <MaskIcon src={PANEL_ART.live} size={16} />
              LIVE
              <MaskIcon src={PANEL_ART.players} size={16} />
              {place.user_count ?? 0}
            </span>
            <span className={styles.eventWhen}>{eventSchedule(event)}</span>
          </div>
        ) : (
          <p className={styles.eventWhen}>{eventSchedule(event)}</p>
        )}
        <h2 className={styles.placeName}>{event.name}</h2>
        <p className={styles.creator}>
          hosted by <b>{event.user_name}</b> - at{' '}
          <b>
            {place.title} ({event.x}, {event.y})
          </b>
        </p>
        {!live && (
          <p className={styles.attendees}>
            <MaskIcon src={PANEL_ART.players} size={20} />
            <b>{event.total_attendees ?? 0}</b> interested
          </p>
        )}
        <div className={styles.eventActions}>
          {live ? (
            <button type="button" className={`${styles.primaryButton} ${styles.jumpIn} ${styles.eventMain}`} onClick={onJumpIn}>
              JUMP IN
              <MaskIcon src={PANEL_ART.jumpIn} size={24} />
            </button>
          ) : (
            <button type="button" className={`${styles.primaryButton} ${styles.remindLarge} ${styles.eventMain}`} aria-pressed={attending} onClick={remind}>
              <MaskIcon src={attending ? PANEL_ART.reminderOn : PANEL_ART.reminderOff} size={24} />
              remind me
            </button>
          )}
          <span className={styles.shareAnchor}>
            <button type="button" className={`${styles.action} ${styles.eventShareLarge}`} aria-label="Share" aria-haspopup="menu" aria-expanded={sharing} onClick={() => setSharing(true)}>
              <MaskIcon src={PANEL_ART.share} size={22} />
            </button>
            {sharing && <ShareMenu links={eventShareLinks(event, referrer)} onClose={() => setSharing(false)} />}
          </span>
        </div>
        <hr className={styles.rule} />
        <h3 className={styles.label}>DESCRIPTION</h3>
        <p className={styles.description}>
          <TextWithLinks text={event.description ?? ''} />
        </p>
        <hr className={styles.rule} />
        <h3 className={`${styles.label} ${styles.labelLarge}`}>TIME TABLE</h3>
        <ul className={styles.schedule}>
          {upcomingDates(event).map((date) => (
            <li key={date.getTime()} className={styles.scheduleRow}>
              <span>{date.toUTCString()}</span>
              <a className={styles.calendar} href={calendarLink(event, date, referrer)} target="_blank" rel="noopener noreferrer">
                add to calendar
              </a>
            </li>
          ))}
        </ul>
      </div>
    </div>
  )
}
