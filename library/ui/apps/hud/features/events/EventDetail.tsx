// The event's detail: its picture, when and where it is, its description and every date it
// repeats on, over a dark scrim. Opened from a card; reads the event from the session, so a
// reminder set here shows on the card too.

import { useEffect } from 'react'
import { TintButton, MaskIcon, openPopup } from '../../design'
import closeIcon from '../../assets/events/close-icn-thin.png'
import calendarAddIcon from '../../assets/events/add-to-calendar-icn.png'
import jumpInIcon from '../../assets/events/jump-in-outline.png'
import pinIcon from '../../assets/events/pin-outline.png'
import timeIcon from '../../assets/events/time-icn.png'
import { useSession } from '../session/SessionContext'
import { EventActionButtons, EventImage } from './EventCard'
import { eventPlaceText, eventTimeText, scheduleDates, scheduleText } from './eventFormat'
import { useEventActions } from './useEventActions'
import styles from './EventDetail.module.css'

function EventDetail({ eventId, onClose }: { eventId: string; onClose: () => void }): React.JSX.Element | null {
  const { store } = useSession().events.data
  const actions = useEventActions()
  const event = store.byId[eventId]
  // the page closed under it and dropped its events
  const gone = event == null
  useEffect(() => {
    if (gone) onClose()
  }, [gone, onClose])
  if (event == null) return null
  const place = event.place_id != null ? store.places[event.place_id] : undefined
  const jumpIn = (): boolean => {
    const travelling = actions.jumpIn(event)
    if (travelling) onClose()
    return travelling
  }
  return (
    <div className={styles.panel} role="dialog" aria-modal="true" aria-label={event.name} onClick={(e) => e.stopPropagation()}>
        <span className={styles.glow} />
        <div className={styles.scroll}>
          <div className={styles.hero}>
            <EventImage src={event.image} className={styles.heroImage} />
            <span className={styles.heroShade} />
            <div className={styles.info}>
              {event.live && <span className={styles.liveBadge}>LIVE</span>}
              <p className={styles.time}>{eventTimeText(event, false)}</p>
              <h2 className={styles.name}>{event.name}</h2>
              <p className={styles.host}>
                Hosted by <b>{event.user_name}</b>
              </p>
              <EventActionButtons event={event} actions={{ ...actions, jumpIn }} size="large" remindLabel variant="detail" className={styles.buttons} />
            </div>
          </div>
          <div className={styles.body}>
            <h3 className={styles.label}>DESCRIPTION</h3>
            <p className={styles.description}>{event.description}</p>
            <hr className={styles.line} />
            <div className={styles.schedules}>
              <MaskIcon src={timeIcon} size={24} className={styles.icon} />
              <ul className={styles.scheduleList}>
                {scheduleDates(event).map((date) => (
                  <li key={date.getTime()}>
                    <button type="button" className={styles.schedule} onClick={() => actions.addToCalendar(event, date)}>
                      <span>{scheduleText(date, event.duration ?? 0)}</span>
                      <MaskIcon src={calendarAddIcon} size={24} className={styles.scheduleIcon} />
                    </button>
                  </li>
                ))}
              </ul>
            </div>
            <hr className={styles.line} />
            <div className={styles.place}>
              <MaskIcon src={pinIcon} size={24} className={styles.icon} />
              <span className={styles.placeName}>{eventPlaceText(event, place)}</span>
              <TintButton radius="button" tone="brand" className={styles.placeJump} aria-label={`Jump in to ${eventPlaceText(event, place)}`} onClick={jumpIn}>
                <span>JUMP IN</span>
                <MaskIcon src={jumpInIcon} size={20} />
              </TintButton>
            </div>
          </div>
        </div>
        <TintButton radius="md" tone="shade" className={styles.close} aria-label="Close event detail" onClick={onClose}>
          <MaskIcon src={closeIcon} size={14} />
        </TintButton>
    </div>
  )
}

export function openEventDetail(eventId: string): void {
  openPopup((close) => <EventDetail eventId={eventId} onClose={close} />, { deep: true })
}
