// The HUD's full-screen pages (Backpack, Places, Events, Map, Gallery, Settings, Communities):
// how each opens (key, rail button, top-bar tab) and the control that proves it is open. Names are
// the HUD's accessible names (react-web/src/features); the key hints in the top-bar tabs are the
// engine's default bindings, so a tab is `Places [ Z ]` as the page joins its text nodes.
import type { Explorer, Selector } from '../driver/index.ts'

export interface Page {
  /** The `KeyboardEvent.code` that toggles the page, when the engine binds one. */
  key?: string
  /** The sidebar rail button's name. */
  rail?: string
  /** The top-bar tab's name. */
  tab?: string
  /** A control only this page shows. */
  marker: Selector
}

export type PageName = 'backpack' | 'communities' | 'gallery' | 'settings' | 'places' | 'events' | 'map'

export const PAGES: Record<PageName, Page> = {
  backpack: { key: 'KeyI', rail: 'Backpack', tab: 'Backpack [ I ]', marker: { name: 'Backpack', role: 'heading' } },
  communities: { key: 'KeyO', rail: 'Communities', marker: { name: 'Communities', role: 'heading' } },
  gallery: { key: 'KeyG', rail: 'Gallery', tab: 'Gallery [ G ]', marker: { name: 'Gallery', role: 'heading' } },
  settings: { key: 'KeyP', rail: 'Settings', tab: 'Settings [ P ]', marker: { name: 'Settings', role: 'heading' } },
  places: { key: 'KeyZ', rail: 'Places', tab: 'Places [ Z ]', marker: { name: 'Places sections', role: 'tablist' } },
  events: { rail: 'Events', tab: 'Events', marker: { name: 'Events sections', role: 'tablist' } },
  map: { key: 'KeyM', tab: 'Map [ M ]', marker: { name: 'Search places & worlds', role: 'textbox' } }
}

export const PAGE_OPEN_TIMEOUT_MS = 20_000

/** Open a page by its key and wait for it. */
export async function openWithKey(explorer: Explorer, page: PageName): Promise<void> {
  const { key, marker } = PAGES[page]
  if (!key) throw new Error(`the ${page} page has no key`)
  await explorer.key(key)
  await explorer.hud.wait(marker, { timeoutMs: PAGE_OPEN_TIMEOUT_MS })
}

/** Open a page from the sidebar rail and wait for it. */
export async function openFromRail(explorer: Explorer, page: PageName): Promise<void> {
  const { rail, marker } = PAGES[page]
  if (!rail) throw new Error(`the ${page} page has no rail button`)
  await explorer.hud.click({ name: rail, role: 'button' })
  await explorer.hud.wait(marker, { timeoutMs: PAGE_OPEN_TIMEOUT_MS })
}

/** Switch to a page with the top bar of an open page. */
export async function switchTab(explorer: Explorer, page: PageName): Promise<void> {
  const { tab, marker } = PAGES[page]
  if (!tab) throw new Error(`the ${page} page has no top-bar tab`)
  await explorer.hud.click({ name: tab, role: 'button' })
  await explorer.hud.wait(marker, { timeoutMs: PAGE_OPEN_TIMEOUT_MS })
}

/** Close the open page with Escape, as explorer-automation's Panel.Close does, and the top bar's
 *  Close button when Escape leaves it open. */
export async function closePage(explorer: Explorer, page: PageName): Promise<void> {
  const { marker } = PAGES[page]
  await explorer.escape()
  try {
    await explorer.hud.waitHidden(marker, 5000)
  } catch {
    await explorer.hud.click({ name: 'Close', role: 'button', index: 0 })
    await explorer.hud.waitHidden(marker, 15_000)
  }
}
