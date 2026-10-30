// explorer-automation `ExplorePanelTests` (7): the sidebar opens each section; the tabs switch
// between them. The Bevy HUD has a page per section instead of one panel with tabs: the rail
// opens a page, and an open page's top bar switches to the others.
//
// | Unity test                      | Here                                             | Class   |
// | TestOpenEventsFromSidebar       | rail Events opens the Events page                | Port    |
// | TestOpenPlacesFromSidebar       | rail Places opens the Places page                | Port    |
// | TestOpenCommunitiesFromSidebar  | rail Communities opens the Communities page      | Port    |
// | TestOpenBackpackFromSidebar     | rail Backpack opens the Backpack page            | Port    |
// | TestOpenGalleryFromSidebar      | rail Gallery opens the Gallery page              | Port    |
// | TestOpenSettingsFromSidebar     | rail Settings opens the Settings page            | Port    |
// | TestSwitchBetweenAllTabs        | the top bar switches Events, Places, Map, Backpack, Gallery, Settings; Communities has no tab | Partial |
import assert from 'node:assert/strict'
import { spec } from '../driver/index.ts'
import { PAGES, closePage, openFromRail, switchTab, type PageName } from './pages.ts'

const scenario = spec(import.meta.url)

const FROM_RAIL: PageName[] = ['events', 'places', 'communities', 'backpack', 'gallery', 'settings']

for (const page of FROM_RAIL) {
  scenario(`the rail's ${PAGES[page].rail} button opens the ${page} page`, async ({ explorer }) => {
    await openFromRail(explorer, page)
    assert.ok(await explorer.hud.isVisible(PAGES[page].marker), `${page} should be open`)
    await closePage(explorer, page)
  })
}

scenario('the top bar switches between every page that has a tab', async ({ explorer }) => {
  await openFromRail(explorer, 'events')
  for (const page of ['places', 'map', 'backpack', 'gallery', 'settings', 'events'] as const) {
    await switchTab(explorer, page)
    assert.ok(await explorer.hud.isVisible(PAGES[page].marker), `${page} should be open after its tab`)
  }
  await closePage(explorer, 'events')
})
