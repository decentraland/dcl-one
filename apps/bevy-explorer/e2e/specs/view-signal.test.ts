// explorer-automation `ViewSignalSmokeTests` (2): a self-test of the Unity harness's view
// lifecycle probe (`MVC.AltTesterViewProbe`), not a user flow. Dropped: the Bevy HUD has no such
// probe, and `/hud_wait` proves a page's open and closed states in every other spec.
//
// | Unity test                             | Here                                   | Class |
// | ExplorePanelReportsShownThenHidden     | a Unity harness self-test              | Drop  |
// | ExplorePanelViewObjectUsesTheSignal    | a Unity harness self-test              | Drop  |
import { spec } from '../driver/index.ts'

const scenario = spec(import.meta.url)

scenario('the explore panel reports shown then hidden', async () => {}, {
  todo: 'a Unity harness self-test of its view probe; the Bevy HUD has none, and /hud_wait covers open and closed states'
})

scenario('a view object waits on the signal', async () => {}, {
  todo: 'a Unity harness self-test of its view probe; the Bevy HUD has none, and /hud_wait covers open and closed states'
})
