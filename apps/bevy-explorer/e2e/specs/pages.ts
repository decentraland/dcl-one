// The HUD's full-screen pages (Backpack, Places, Events, Map, Gallery, Settings, Communities):
// how each opens (key, rail button, top-bar tab) and the control that proves it is open. Names are
// the HUD's accessible names (library/ui/apps/hud/features); the key hints in the top-bar tabs are the
// engine's default bindings, so a tab is `Places [ Z ]` as the page joins its text nodes.
import type { Explorer, Selector } from '../driver/index.ts'

/** A community card's name without the initial its cover shows for an image that did not load,
 *  which shows only once the image fails: the initial is dropped when the next word starts with it. */
export function withoutInitial(cardName: string): string {
  const words = cardName.split(' ')
  if (words.length > 1 && words[0]!.length === 1 && words[1]!.toLowerCase().startsWith(words[0]!.toLowerCase())) words.shift()
  return words.join(' ')
}

/** The community card a name read earlier (`withoutInitial`) names, with or without its initial. */
export function cardSelector(card: string): Selector {
  return { name: card, role: 'button', exact: false }
}

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
  events: { key: 'KeyX', rail: 'Events', tab: 'Events [ X ]', marker: { name: 'Events', role: 'heading' } },
  map: { key: 'KeyM', tab: 'Map [ M ]', marker: { name: 'Search places & worlds', role: 'textbox' } }
}

export const PAGE_OPEN_TIMEOUT_MS = 20_000
/** The most an Escape is given to close a page, which takes a third of a second: a ceiling for a
 *  press the page did not take, before the top bar's Close button. */
const PAGE_CLOSE_TIMEOUT_MS = 10_000

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

/** Close the open page with Escape, as explorer-automation's Panel.Close does. While a field of
 *  the page holds the keyboard (the engine says so) no action resolves: the first press only
 *  leaves the field, so it waits for the engine to have the keyboard back, and the next closes the
 *  page. The top bar's Close button closes a page the Escape did not, and one whose field keeps
 *  the keys through a press (the map's search spends its first Escape clearing itself). Closing
 *  the Backpack deploys the look it changed, so it waits out the catalyst's window since the last
 *  deployment first. */
export async function closePage(explorer: Explorer, page: PageName): Promise<void> {
  const { marker } = PAGES[page]
  if (page === 'backpack') await explorer.profileDeployWindow()
  try {
    if ((await explorer.worldState()).keyboardClaimed === true) {
      await explorer.escape()
      await explorer.keyboardReleased()
    }
    await explorer.escape()
    await explorer.hud.waitHidden(marker, PAGE_CLOSE_TIMEOUT_MS)
    return
  } catch {
    // still open: a field kept the keys, or the page did not take the press
  }
  await explorer.hud.click({ name: 'Close', role: 'button', index: 0 })
  await explorer.hud.waitHidden(marker, 15_000)
}
