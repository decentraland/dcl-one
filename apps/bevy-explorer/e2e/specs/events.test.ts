// explorer-automation `EventsTests` (3): the Events section's day switcher, event detail and
// header buttons. The page opens on a calendar of day columns under a bar of day names; a day's
// name opens that day's events, and the header's Today returns to the calendar. An event card is
// a button named `Event: <name>`, and opens a dialog named by the event. The events are the
// service's own, so the detail scenario reads the first card it finds and has nothing to open on a
// calendar with no events.
//
// | Unity test                      | Here                                                                 | Class   |
// | TestSwitchEventsDay             | Tomorrow opens the day's list; Today returns to the calendar         | Port    |
// | TestOpenEventDetail             | the first event card opens a dialog with its name, when there is one | Partial |
// | TestCreateEventButtonPresent    | the header offers CREATE EVENT and Today                             | Port    |
import assert from 'node:assert/strict'
import { spec, type Explorer } from '../driver/index.ts'
import { PAGES, closePage, openFromRail } from './pages.ts'

const scenario = spec(import.meta.url)

/** The events come from the events service, read through the bridge. */
const EVENTS_TIMEOUT_MS = 40_000
const TEXT_TIMEOUT_MS = 10_000

const TODAY = { name: 'Go to today', role: 'button' } as const
const CREATE = { name: 'CREATE EVENT', role: 'button' } as const
const NEXT_DAYS = { name: 'Next days', role: 'button' } as const
const TOMORROW = { name: 'Tomorrow', role: 'button' } as const
const BACK = { name: 'Back to the calendar', role: 'button' } as const
const CARD_PREFIX = 'Event: '

/** The calendar has read its events once its range arrows take a press. */
async function calendarLoaded(explorer: Explorer): Promise<void> {
  await explorer.hud.wait(NEXT_DAYS, { state: 'actionable', timeoutMs: EVENTS_TIMEOUT_MS })
}

scenario('a day name opens that day of events and Today returns to the calendar', async ({ explorer }) => {
  await openFromRail(explorer, 'events')
  await calendarLoaded(explorer)
  await explorer.hud.click(TOMORROW)
  // the day's heading is "Tomorrow", then "Tomorrow (N)" once its events are read
  await explorer.hud.wait({ name: 'Tomorrow (', role: 'heading', exact: false }, { timeoutMs: EVENTS_TIMEOUT_MS })
  assert.ok(await explorer.hud.isVisible(BACK), 'the day view should offer the way back')
  assert.equal(await explorer.hud.isVisible(NEXT_DAYS), false, 'the calendar should be gone while a day is open')
  await explorer.hud.click(TODAY)
  await calendarLoaded(explorer)
  assert.ok(await explorer.hud.isVisible(TOMORROW), 'the calendar should show its day names again')
  await closePage(explorer, 'events')
})

scenario('an event card opens the event detail with the same name', async ({ explorer }) => {
  await openFromRail(explorer, 'events')
  await calendarLoaded(explorer)
  const cards = (await explorer.hud.list('button')).map((button) => button.name).filter((name) => name.startsWith(CARD_PREFIX))
  if (cards.length === 0) {
    // no event in the days shown: the calendar offers its empty slots instead
    assert.ok(await explorer.hud.isVisible({ name: 'Create event on this day', role: 'button', index: 0 }), 'an empty calendar should offer its slots')
    await closePage(explorer, 'events')
    return
  }
  const name = cards[0]!.slice(CARD_PREFIX.length)
  await explorer.hud.click({ name: cards[0]!, role: 'button', index: 0 })
  const detail = { name, role: 'dialog' } as const
  await explorer.hud.wait(detail, { timeoutMs: TEXT_TIMEOUT_MS })
  await explorer.hud.click({ name: 'Close event detail', role: 'button' })
  await explorer.hud.wait(detail, { state: 'hidden', timeoutMs: TEXT_TIMEOUT_MS })
  await closePage(explorer, 'events')
})

scenario('the Events header offers CREATE EVENT and Today', async ({ explorer }) => {
  await openFromRail(explorer, 'events')
  await explorer.hud.wait(PAGES.events.marker, { timeoutMs: TEXT_TIMEOUT_MS })
  assert.ok(await explorer.hud.isVisible(CREATE), 'the header should offer CREATE EVENT')
  assert.ok(await explorer.hud.isVisible(TODAY), 'the header should offer Today')
  await closePage(explorer, 'events')
})
