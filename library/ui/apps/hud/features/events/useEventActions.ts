// What a card's and the detail's buttons do: the reminder, the calendar and share links, the
// jump, and the analytics each sends.

import { useMemo } from 'react'
import { showToast } from '../../design'
import type { EventData } from '@explorer-core/engine/protocol'
import { eventDestination } from '@explorer-core/features/events/eventsApi'
import { track } from '@explorer-core/lib/analytics'
import { useSession } from '../session/SessionContext'
import { eventAnalytics, eventCalendarUrl, eventCopyLink, eventShareUrl, createEventUrl, type EventLinkSource } from './eventFormat'

export interface EventActions {
  remind: (event: EventData) => void
  reminding: (event: EventData) => boolean
  addToCalendar: (event: EventLinkSource, start?: Date) => void
  /** Travel to the event and close the Events page. False when nothing happened: the event names
   *  no world to travel to, or the startup lobby is not taking picks yet. */
  jumpIn: (event: EventData) => boolean
  shareOnX: (event: EventLinkSource) => void
  copyLink: (event: EventLinkSource) => void
  createEvent: (fromHeader: boolean) => void
}

const openUrl = (url: string): void => {
  window.open(url, '_blank', 'noopener')
}

export function useEventActions(): EventActions {
  const session = useSession()
  const { data, close } = session.events
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
        const dest = eventDestination(event)
        if (dest == null) {
          showToast('This event’s world cannot be reached.', { tone: 'error' })
          return false
        }
        const outcome = dest.kind === 'world' ? changeRealm(dest.realm) : teleportToPlace(dest.x, dest.y)
        if (outcome === 'refused') return false
        track('event_jumped_in', eventAnalytics(event))
        // not a toggle: over the startup lobby the pick has closed the page already
        close()
        return true
      },
      shareOnX: (event) => {
        openUrl(eventShareUrl(event, referrer))
        track('event_shared', eventAnalytics(event))
      },
      copyLink: (event) => {
        const failed = (): void => showToast('Could not copy the link', { tone: 'error' })
        if (navigator.clipboard == null) return failed()
        navigator.clipboard.writeText(eventCopyLink(event, referrer)).then(() => {
          showToast('Link copied to clipboard!')
          track('event_link_copied', eventAnalytics(event))
        }, failed)
      },
      createEvent: (fromHeader) => {
        openUrl(createEventUrl())
        track('event_creation_opened', { from_header_button: fromHeader })
      }
    }),
    [data, pending, referrer, changeRealm, teleportToPlace, close]
  )
}
