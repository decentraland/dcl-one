// explorer-automation `ShortcutsTests` (7): a key opens its page, Escape closes it.
//
// | Unity test                          | Here                                   | Class   |
// | TestOpenEventsWithShortcut (X)      | no key: the rail opens Events          | Gap     |
// | TestOpenPlacesWithShortcut (Z)      | Z opens Places                         | Port    |
// | TestOpenCommunitiesWithShortcut (O) | O opens Communities                    | Port    |
// | TestOpenMapWithShortcut (M)         | M opens the Map                        | Port    |
// | TestOpenBackpackWithShortcut (I)    | I opens the Backpack                   | Port    |
// | TestOpenGalleryWithShortcut (K)     | G opens the Gallery (the engine's key) | Partial |
// | TestOpenSettingsWithShortcut (P)    | P opens Settings                       | Port    |
import assert from 'node:assert/strict'
import { spec } from '../driver/index.ts'
import { PAGES, closePage, openWithKey, type PageName } from './pages.ts'

const scenario = spec(import.meta.url)

const WITH_KEY: { page: PageName; unity: string }[] = [
  { page: 'places', unity: 'Z' },
  { page: 'communities', unity: 'O' },
  { page: 'map', unity: 'M' },
  { page: 'backpack', unity: 'I' },
  { page: 'gallery', unity: 'K' },
  { page: 'settings', unity: 'P' }
]

for (const { page, unity } of WITH_KEY) {
  const key = PAGES[page].key!
  scenario(`${key} opens ${page} and Escape closes it (Unity: ${unity})`, async ({ explorer }) => {
    await openWithKey(explorer, page)
    assert.ok(await explorer.hud.isVisible(PAGES[page].marker), `${page} should be open after ${key}`)
    await closePage(explorer, page)
    assert.equal(await explorer.hud.isVisible(PAGES[page].marker), false, `${page} should be closed after Escape`)
  })
}

scenario('X opens Events', async () => {}, {
  todo: 'the Bevy HUD binds no key to Events; the rail button opens it (explore-panel spec)'
})
