// explorer-automation `EventsTests` (3): the Events section's day switcher, event detail and
// header buttons. The Bevy Events page is two lists of cards (Live now, Upcoming): it has no
// calendar, day selector, Today or CREATE EVENT control, and a card jumps to the event's parcel or
// world instead of opening a detail popup, so every test is a Gap. Opening the page is covered by
// explore-panel.test.ts.
//
// | Unity test                      | Here                                          | Class |
// | TestSwitchEventsDay             | no day selector, per-day list or Today button | Gap   |
// | TestOpenEventDetail             | a card jumps in; there is no detail popup     | Gap   |
// | TestCreateEventButtonPresent    | no CREATE EVENT or Today button               | Gap   |
import { spec } from '../driver/index.ts'

const scenario = spec(import.meta.url)

scenario('a day selector switches to one day of events and Today returns', async () => {}, {
  todo: 'the Bevy Events page has no calendar, day selector, per-day list or Today button; it has the Live now and Upcoming tabs'
})

scenario('an event card opens the event detail with the same name', async () => {}, {
  todo: 'an event card on the Bevy Events page jumps to the event and closes the page; there is no detail popup'
})

scenario('the Events header offers CREATE EVENT and Today', async () => {}, {
  todo: 'the Bevy Events page has no header buttons, only the Live now and Upcoming tabs'
})
