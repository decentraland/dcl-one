// DOMAIN: the Events page — the calendar, a day's grid, the cards' buttons and the detail, driven
// through the session and the bridge messages it sends.
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PopupHost, ToastHost, resetPopups } from '../design'
import type { EventData, GetEventsRequest } from '../engine/protocol'
import { EventsPage } from '../features/events/EventsPage'
import { CAROUSEL_DELAY_MS } from '../features/events/EventsCalendar'
import { addDays, calendarRange, dayRange, startOfDay } from '../features/events/eventFormat'
import { EVENTS_REPLY_TIMEOUT_MS } from '../features/events/useEventsData'
import { SessionProvider } from '../features/session/SessionContext'
import { useEngineSession, type EngineSession } from '../features/session/useEngineSession'
import { Sidebar } from '../features/sidebar/Sidebar'
import { pending } from '../lib/analytics'
import { BASE_DOMAIN } from '../lib/baseDomain'
import { eventAt } from './eventFixtures'
import { FakeDriver, fakeSession } from './harness'

// a Tuesday noon, local time: the calendar's "today"
const NOW = new Date(2026, 8, 8, 12, 0, 0)
const today = startOfDay(NOW)
const at = (dayOffset: number, hour: number, minute = 0): Date => new Date(today.getFullYear(), today.getMonth(), today.getDate() + dayOffset, hour, minute)

const FRIEND = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'

interface Hud {
  driver: FakeDriver
  session: () => EngineSession
  /** Answer the request of a list still waiting, by what it asked for. */
  answer: (which: 'range' | 'live' | 'highlight', events: EventData[] | 'failed') => void
  open: () => void
}

function renderHud(): Hud {
  const driver = new FakeDriver()
  let current!: EngineSession
  function Tree(): React.JSX.Element {
    const session = useEngineSession(() => driver)
    current = session
    return (
      <SessionProvider value={session}>
        <EventsPage events={session.events} profile={session.profile} friends={session.friends} onNavigate={vi.fn()} />
        <PopupHost />
        <ToastHost />
      </SessionProvider>
    )
  }
  render(<Tree />)
  const is = (which: 'range' | 'live' | 'highlight', m: GetEventsRequest): boolean => (which === 'range' ? m.list == null : m.list === which)
  const answered = new Set<number>()
  return {
    driver,
    session: () => current,
    answer: (which, events) => {
      const request = [...driver.sentOf('getEvents')].reverse().find((m) => is(which, m) && !answered.has(m.request))
      if (request == null) throw new Error(`no ${which} request is waiting`)
      answered.add(request.request)
      driver.emit(events === 'failed' ? { kind: 'events', request: request.request, events: [], failed: true } : { kind: 'events', request: request.request, events })
    },
    open: () => act(() => current.events.toggle('sidebar'))
  }
}

/** Open the page and answer its three first reads. */
function openWith(range: EventData[], extra: { live?: EventData[]; highlighted?: EventData[] } = {}): Hud {
  const hud = renderHud()
  hud.open()
  hud.answer('highlight', extra.highlighted ?? [])
  hud.answer('live', extra.live ?? [])
  hud.answer('range', range)
  return hud
}

const card = (name: string): HTMLElement => screen.getByRole('button', { name: `Event: ${name}` }).closest('article')!
// the analytics queue outlives a test: each reads what was queued since it began
let queuedBefore = 0
const queued = (): ReturnType<typeof pending> => pending().slice(queuedBefore)
const sentEvents = (names: string[]): string[] => queued().map((e) => e.event).filter((e) => names.includes(e))

beforeEach(() => {
  queuedBefore = pending().length
  vi.useFakeTimers({ toFake: ['Date'] })
  vi.setSystemTime(NOW)
  vi.stubGlobal('open', vi.fn())
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
  resetPopups()
  document.body.innerHTML = ''
})

describe('events: the sidebar', () => {
  it('shows Events after Notifications with the live count badge and its key', async () => {
    vi.stubGlobal('fetch', vi.fn(() => Promise.resolve(new Response(JSON.stringify({ ok: true, data: [{ id: 'a' }, { id: 'b' }] })))))
    const s = fakeSession()
    render(<Sidebar session={s} />)
    const labels = screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))
    expect(labels[labels.indexOf('Notifications') + 1]).toBe('Events')
    const button = screen.getByRole('button', { name: 'Events' })
    await waitFor(() => expect(within(button).getByText('2')).toBeInTheDocument())
    await userEvent.click(button)
    expect(vi.mocked(s.events.toggle)).toHaveBeenCalledWith('sidebar')
  })
})

describe('events: the calendar', () => {
  it('reads five days from today, the live events and the highlighted ones, signed through the bridge', () => {
    const hud = renderHud()
    hud.open()
    const sent = hud.driver.sentOf('getEvents')
    expect(sent).toHaveLength(3)
    expect(sent.find((m) => m.list == null)).toMatchObject(calendarRange(today, 5))
    expect(sent.some((m) => m.list === 'live')).toBe(true)
    expect(sent.find((m) => m.list === 'highlight')).toMatchObject({ limit: 10, offset: 0 })
    expect(screen.getByRole('heading', { name: 'Events' })).toBeInTheDocument()
    // loading: every column is busy, and the range arrows wait
    expect(document.querySelectorAll('[aria-busy="true"]')).toHaveLength(5)
    expect(screen.getByRole('button', { name: 'Next days' })).toBeDisabled()
  })

  it('shows each event under its day, big when live, reminded or of a community the user is in', () => {
    const hud = openWith(
      [
        eventAt('live', at(0, 11), { name: 'Live show', live: true }),
        eventAt('plain', at(0, 18), { name: 'Plain talk' }),
        eventAt('reminded', at(1, 10), { name: 'Reminded meetup', attending: true }),
        eventAt('mine', at(2, 10), { name: 'My community night', community_id: 'c1' }),
        eventAt('theirs', at(2, 12), { name: 'Their community night', community_id: 'c9' })
      ],
      { live: [eventAt('old', at(-1, 9), { name: 'Since yesterday', live: true })] }
    )
    hud.driver.emit({ kind: 'communityChannels', communities: [{ id: 'c1', name: 'Builders', thumbnail: '', role: 'member' }] })
    expect(['Today', 'Tomorrow', 'Thu, Sep 10', 'Fri, Sep 11', 'Sat, Sep 12'].every((name) => screen.getByRole('button', { name }) != null)).toBe(true)
    const order = [...document.querySelectorAll('article')].map((a) => a.getAttribute('data-event'))
    expect(order).toEqual(['live', 'old', 'plain', 'reminded', 'mine', 'theirs'])
    expect(within(card('Live show')).getByText('Started 1 hour ago')).toBeInTheDocument()
    expect(within(card('Plain talk')).getByText('6:00PM')).toBeInTheDocument()
    // the community's name shows only for the one the user is a member of
    expect(within(card('My community night')).getByRole('tooltip')).toHaveTextContent('Builders')
    expect(within(card('Their community night')).queryByRole('tooltip')).toBeNull()
    // live: jump in, no reminder; to come: the reminder and the calendar, no jump
    expect(within(card('Live show')).getByRole('button', { name: /jump in/i })).toBeInTheDocument()
    expect(within(card('Live show')).queryByRole('button', { name: 'Remind me' })).toBeNull()
    expect(within(card('Plain talk')).getByRole('button', { name: 'Remind me' })).toHaveAttribute('aria-pressed', 'false')
    expect(within(card('Reminded meetup')).getByRole('button', { name: 'Remind me' })).toHaveAttribute('aria-pressed', 'true')
    expect(within(card('Plain talk')).queryByRole('button', { name: /jump in/i })).toBeNull()
  })

  it('steps the range by the days shown, and cannot go before today', async () => {
    const hud = openWith([])
    expect(screen.getByRole('button', { name: 'Previous days' })).toBeDisabled()
    await userEvent.click(screen.getByRole('button', { name: 'Next days' }))
    expect(hud.driver.last('getEvents')).toMatchObject(calendarRange(addDays(today, 5), 5))
    // only a calendar that starts today reads the live events
    expect(hud.driver.sentOf('getEvents').filter((m) => m.list === 'live')).toHaveLength(1)
    hud.answer('range', [])
    expect(screen.getByRole('button', { name: 'Sun, Sep 13' })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Previous days' }))
    hud.answer('live', [])
    hud.answer('range', [])
    expect(screen.getByRole('button', { name: 'Previous days' })).toBeDisabled()
    await userEvent.click(screen.getByRole('button', { name: 'Next days' }))
    hud.answer('range', [])
    await userEvent.click(screen.getByRole('button', { name: 'Go to today' }))
    expect(hud.driver.sentOf('getEvents').filter((m) => m.list == null).at(-1)).toMatchObject(calendarRange(today, 5))
  })

  it('shows an error with Retry when the read fails, and reads again', async () => {
    const hud = renderHud()
    hud.open()
    hud.answer('highlight', [])
    hud.answer('live', [])
    hud.answer('range', 'failed')
    expect(screen.getAllByText('There was an error loading events. Please try again.').length).toBeGreaterThan(0)
    expect(document.querySelectorAll('[aria-busy="true"]')).toHaveLength(0)
    const before = hud.driver.sentOf('getEvents').length
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(hud.driver.sentOf('getEvents').length).toBe(before + 3)
    hud.answer('highlight', [])
    hud.answer('live', [])
    hud.answer('range', [eventAt('a', at(0, 18), { name: 'Back again' })])
    expect(screen.getByRole('button', { name: 'Event: Back again' })).toBeInTheDocument()
  })

  it('counts a read the bridge never answers as failed, and still takes a late answer', () => {
    vi.useFakeTimers()
    vi.setSystemTime(NOW)
    const hud = renderHud()
    hud.open()
    act(() => void vi.advanceTimersByTime(EVENTS_REPLY_TIMEOUT_MS))
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument()
    hud.answer('range', [eventAt('a', at(0, 18), { name: 'Late answer' })])
    expect(screen.getByRole('button', { name: 'Event: Late answer' })).toBeInTheDocument()
  })

  it('offers an empty slot that opens the create-event page', async () => {
    openWith([])
    const slots = screen.getAllByRole('button', { name: 'Create event on this day' })
    expect(slots).toHaveLength(15)
    await userEvent.click(slots[0])
    expect(String(vi.mocked(window.open).mock.calls[0][0])).toContain(`https://${BASE_DOMAIN}/whats-on/new-event?`)
    await userEvent.click(screen.getByRole('button', { name: 'CREATE EVENT' }))
    expect(queued().filter((e) => e.event === 'event_creation_opened').map((e) => e.properties.from_header_button)).toEqual([false, true])
  })
})

describe('events: the highlighted carousel', () => {
  const highlighted = [eventAt('h1', at(2, 10), { name: 'Festival', highlighted: true }), eventAt('h2', at(3, 10), { name: 'Art week', highlighted: true })]

  it('takes the last day\'s place and moves on by itself', () => {
    vi.useFakeTimers()
    vi.setSystemTime(NOW)
    openWith([], { highlighted })
    expect(screen.queryByRole('button', { name: 'Sat, Sep 12' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Fri, Sep 11' })).toBeInTheDocument()
    const carousel = screen.getByRole('region', { name: 'Highlighted events' })
    expect(within(carousel).getByRole('button', { name: 'Event: Festival' })).toBeInTheDocument()
    expect(within(carousel).getByText(/Organized by/)).toHaveTextContent('Organized by Host')
    expect(within(carousel).getByRole('button', { name: 'Highlighted event 1 of 2' })).toHaveAttribute('aria-current', 'true')
    act(() => void vi.advanceTimersByTime(CAROUSEL_DELAY_MS))
    expect(within(carousel).getByRole('button', { name: 'Event: Art week' })).toBeInTheDocument()
    fireEvent.click(within(carousel).getByRole('button', { name: 'Highlighted event 1 of 2' }))
    expect(within(carousel).getByRole('button', { name: 'Event: Festival' })).toBeInTheDocument()
  })

  it('shows the friends at a live highlighted event once the friends list arrives', () => {
    const hud = openWith([], { highlighted: [eventAt('h1', at(0, 11), { name: 'Festival', highlighted: true, live: true, connected_addresses: [FRIEND, '0xother'] })] })
    const carousel = screen.getByRole('region', { name: 'Highlighted events' })
    expect(within(carousel).queryByRole('tooltip')).toBeNull()
    expect(within(carousel).getByLabelText('2 here now')).toBeInTheDocument()
    hud.driver.emit({ kind: 'friends', available: true, friends: [{ address: FRIEND, name: 'Mojito', status: 'online' }], received: [], sent: [], blocked: [] })
    expect(within(carousel).getByRole('tooltip')).toHaveTextContent('Mojito')
  })
})

describe('events: a day', () => {
  it('opens from the day\'s name, counts its events and goes back to the calendar', async () => {
    const hud = openWith([])
    await userEvent.click(screen.getByRole('button', { name: 'Tomorrow' }))
    expect(hud.driver.last('getEvents')).toMatchObject(dayRange(addDays(today, 1)))
    expect(screen.getByRole('heading', { name: 'Tomorrow' })).toBeInTheDocument()
    hud.driver.emit({ kind: 'events', request: hud.driver.last('getEvents')!.request, events: [eventAt('a', at(1, 10), { name: 'Morning run' }), eventAt('b', at(1, 20), { name: 'Night run' })] })
    expect(screen.getByRole('heading', { name: 'Tomorrow (2)' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Event: Morning run' })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Back to the calendar' }))
    expect(hud.driver.sentOf('getEvents').filter((m) => m.list == null && m.from === calendarRange(today, 5).from).length).toBe(2)
    expect(screen.getByRole('button', { name: 'Next days' })).toBeInTheDocument()
  })

  it('offers the next day when the day has no events', async () => {
    const hud = openWith([])
    await userEvent.click(screen.getByRole('button', { name: 'Thu, Sep 10' }))
    hud.driver.emit({ kind: 'events', request: hud.driver.last('getEvents')!.request, events: [] })
    expect(screen.getByText('No plans here yet')).toBeInTheDocument()
    expect(screen.getByText('Skip ahead and see what’s coming up next.')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'GO TO NEXT DAY' }))
    expect(hud.driver.last('getEvents')).toMatchObject(dayRange(addDays(today, 3)))
    expect(screen.getByRole('heading', { name: 'Fri, Sep 11' })).toBeInTheDocument()
  })

  it('shows an error with Retry when the day cannot be read', async () => {
    const hud = openWith([])
    await userEvent.click(screen.getByRole('button', { name: 'Tomorrow' }))
    hud.driver.emit({ kind: 'events', request: hud.driver.last('getEvents')!.request, events: [], failed: true })
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(hud.driver.sentOf('getEvents').filter((m) => m.from === dayRange(addDays(today, 1)).from)).toHaveLength(2)
  })

  it('has working buttons on its cards: the reminder, the calendar, share and the jump', async () => {
    const hud = openWith([])
    await userEvent.click(screen.getByRole('button', { name: 'Today' }))
    hud.driver.emit({
      kind: 'events',
      request: hud.driver.last('getEvents')!.request,
      events: [eventAt('up', at(0, 18), { name: 'Evening set' }), eventAt('live', at(0, 11), { name: 'Live set', live: true, x: 5, y: 7 })]
    })
    await userEvent.click(within(card('Evening set')).getByRole('button', { name: 'Remind me' }))
    expect(hud.driver.last('setEventAttending')).toEqual({ kind: 'setEventAttending', eventId: 'up', attending: true })
    hud.driver.emit({ kind: 'eventAttending', eventId: 'up', attending: true })
    expect(within(card('Evening set')).getByRole('button', { name: 'Remind me' })).toHaveAttribute('aria-pressed', 'true')
    await userEvent.click(within(card('Evening set')).getByRole('button', { name: 'Add to calendar' }))
    expect(String(vi.mocked(window.open).mock.calls.at(-1)![0])).toContain('https://calendar.google.com/calendar/r/eventedit?')
    await userEvent.click(within(card('Live set')).getByRole('button', { name: /jump in/i }))
    expect(hud.driver.sent.some((m) => m.kind === 'teleport' && m.x === 5 && m.y === 7)).toBe(true)
    expect(hud.session().events.open).toBe(false)
  })
})

describe('events: the reminder', () => {
  it('changes when the service confirms, on the card and in the detail alike', async () => {
    const hud = openWith([eventAt('a', at(1, 18), { name: 'Evening set' })])
    const remind = (): HTMLElement => within(card('Evening set')).getByRole('button', { name: 'Remind me' })
    await userEvent.click(remind())
    expect(remind()).toHaveAttribute('aria-pressed', 'false')
    expect(remind()).toHaveAttribute('aria-busy', 'true')
    hud.driver.emit({ kind: 'eventAttending', eventId: 'a', attending: true })
    expect(remind()).toHaveAttribute('aria-pressed', 'true')
    // the card keeps its size: only its background says it is reminded
    expect(within(card('Evening set')).queryByText('REMIND ME')).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Event: Evening set' }))
    const detail = screen.getByRole('dialog', { name: 'Evening set' })
    const inDetail = within(detail).getByRole('button', { name: 'Remind me' })
    expect(inDetail).toHaveAttribute('aria-pressed', 'true')
    await userEvent.click(inDetail)
    expect(hud.driver.last('setEventAttending')).toMatchObject({ eventId: 'a', attending: false })
    hud.driver.emit({ kind: 'eventAttending', eventId: 'a', attending: false })
    expect(within(detail).getByRole('button', { name: 'Remind me' })).toHaveAttribute('aria-pressed', 'false')
    expect(remind()).toHaveAttribute('aria-pressed', 'false')
    expect(sentEvents(['event_set_as_interested'])).toHaveLength(2)
  })

  it('says so when the service refuses, and leaves the reminder as it was', async () => {
    const hud = openWith([eventAt('a', at(1, 18), { name: 'Evening set' })])
    await userEvent.click(within(card('Evening set')).getByRole('button', { name: 'Remind me' }))
    hud.driver.emit({ kind: 'eventAttending', eventId: 'a', attending: true, failed: true })
    expect(screen.getByText('There was an error changing your interest on the event. Please try again.')).toBeInTheDocument()
    expect(within(card('Evening set')).getByRole('button', { name: 'Remind me' })).toHaveAttribute('aria-pressed', 'false')
  })
})

describe('events: the detail', () => {
  const weekly = eventAt('w', at(1, 18), {
    name: 'Weekly poker',
    description: 'Bring chips.',
    user_name: 'Dealer',
    place_id: 'p1',
    recurrent: true,
    recurrent_dates: [at(1, 18), at(8, 18)].map((d) => d.toISOString())
  })

  it('shows when, who, what and where, and each date the event repeats on', async () => {
    const hud = openWith([weekly])
    expect(hud.driver.last('getEventPlaces')).toMatchObject({ placeIds: ['p1'] })
    hud.driver.emit({ kind: 'eventPlaces', request: hud.driver.last('getEventPlaces')!.request, places: [{ id: 'p1', title: 'Casino', world_name: null, base_position: '10,-20' }] })
    await userEvent.click(screen.getByRole('button', { name: 'Event: Weekly poker' }))
    const detail = within(screen.getByRole('dialog', { name: 'Weekly poker' }))
    expect(detail.getByText('WED, SEP 09 @ 6:00PM')).toBeInTheDocument()
    expect(detail.getByText(/Hosted by/)).toHaveTextContent('Hosted by Dealer')
    expect(detail.getByText('DESCRIPTION')).toBeInTheDocument()
    expect(detail.getByText('Bring chips.')).toBeInTheDocument()
    expect(detail.getByText('Casino (10,-20)')).toBeInTheDocument()
    const dates = detail.getAllByRole('button', { name: /from 06:00pm to 08:00pm/ })
    expect(dates).toHaveLength(2)
    await userEvent.click(dates[1])
    const url = new URL(String(vi.mocked(window.open).mock.calls.at(-1)![0]))
    expect(url.searchParams.get('dates')).toBe(`${stamp(at(8, 18))}/${stamp(at(8, 20))}`)
    await userEvent.click(detail.getByRole('button', { name: 'Close event detail' }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('jumps in from its place row, closing itself and the page', async () => {
    const hud = openWith([{ ...weekly, world: true, server: 'casino.dcl.eth' }])
    await userEvent.click(screen.getByRole('button', { name: 'Event: Weekly poker' }))
    await userEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: /^Jump in to/ }))
    expect(hud.driver.last('changeRealm')).toMatchObject({ realm: 'casino.dcl.eth' })
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(hud.session().events.open).toBe(false)
    expect(sentEvents(['event_card_clicked', 'event_jumped_in'])).toEqual(['event_card_clicked', 'event_jumped_in'])
  })
})

describe('events: sharing', () => {
  it('copies the link and shares on X from the share menu', async () => {
    const writeText = vi.fn(() => Promise.resolve())
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } })
    openWith([eventAt('a', at(1, 18), { name: 'Evening set' })])
    await userEvent.click(within(card('Evening set')).getByRole('button', { name: 'Share' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'Copy link' }))
    expect(writeText).toHaveBeenCalledWith(`https://${BASE_DOMAIN}/whats-on/?id=a`)
    expect(screen.getByText('Link copied to clipboard!')).toBeInTheDocument()
    expect(screen.queryByRole('menu')).toBeNull()
    await userEvent.click(within(card('Evening set')).getByRole('button', { name: 'Share' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'Share on X' }))
    expect(String(vi.mocked(window.open).mock.calls.at(-1)![0])).toContain('https://twitter.com/intent/tweet?text=Evening%20set&hashtags=DCLPlace')
    expect(sentEvents(['event_link_copied', 'event_shared'])).toEqual(['event_link_copied', 'event_shared'])
  })
})

describe('events: analytics', () => {
  it('reports what opened the page and each day opened', async () => {
    openWith([])
    await userEvent.click(screen.getByRole('button', { name: 'Tomorrow' }))
    const sent = queued().filter((e) => e.event === 'events_section_opened' || e.event === 'events_by_day_opened')
    expect(sent).toHaveLength(2)
    expect(sent[0]).toMatchObject({ event: 'events_section_opened', properties: { source: 'sidebar' } })
    expect(sent[1]).toMatchObject({ event: 'events_by_day_opened', properties: { date: '2026-09-09' } })
  })
})

function stamp(date: Date): string {
  return `${date.toISOString().slice(0, 19).replace(/[-:]/g, '')}Z`
}
