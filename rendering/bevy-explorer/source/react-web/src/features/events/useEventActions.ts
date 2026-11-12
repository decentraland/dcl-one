// What a card's and the detail's buttons do: the reminder, the calendar and share links, the
// jump, and the analytics each sends.

import { useMemo } from 'react'
import { showToast } from '../../design'
import type { EventData } from '../../engine/protocol'
import { track } from '../../lib/analytics'
import { useSession } from '../session/SessionContext'
import { eventAnalytics, eventCalendarUrl, eventCopyLink, eventShareUrl, createEventUrl } from './eventFormat'

export interface EventActions {
  /** Set the reminder, or clear it when set. */
  remind: (event: EventData) => void
  /** Whether the event's reminder change is on its way. */
  reminding: (event: EventData) => boolean
  /** Google Calendar's new-event page for the next occurrence, or the one at `start`. */
  addToCalendar: (event: EventData, start?: Date) => void
  /** Travel to the event and close the Events page. */
  jumpIn: (event: EventData) => void
  shareOnX: (event: EventData) => void
  copyLink: (event: EventData) => void
  createEvent: (fromHeader: boolean) => void
}

const openUrl = (url: string): void => {
  window.open(url, '_blank', 'noopener')
}

export function useEventActions(): EventActions {
  const session = useSession()
  const { data, open, toggle } = session.events
  const { changeRealm, teleportToPlace } = session.map
  const referrer = session.profile.data?.address ?? null
  const pending = data.store.pending
  return useMemo(
    () => ({
      remind: (event) => data.setAttending(event, !event.attending),
      reminding: (event) => pending.includes(event.id),
      addToCalendar: (event, start) => {
        const url = eventCalendarUrl(event, referrer, start)
        if (url == null) return
        openUrl(url)
        track('event_added_to_calendar', eventAnalytics(event))
      },
      jumpIn: (event) => {
        if (event.world) {
          if (event.server == null || event.server === '') return
          changeRealm(event.server)
        } else teleportToPlace(event.x, event.y)
        track('event_jumped_in', eventAnalytics(event))
        if (open) toggle()
      },
      shareOnX: (event) => {
        openUrl(eventShareUrl(event, referrer))
        track('event_shared', eventAnalytics(event))
      },
      copyLink: (event) => {
        void navigator.clipboard?.writeText(eventCopyLink(event, referrer))
        showToast('Link copied to clipboard!')
        track('event_link_copied', eventAnalytics(event))
      },
      createEvent: (fromHeader) => {
        openUrl(createEventUrl())
        track('event_creation_opened', { from_header_button: fromHeader })
      }
    }),
    [data, pending, referrer, changeRealm, teleportToPlace, open, toggle]
  )
}
