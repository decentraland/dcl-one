// QA Automation Suite, High End Functionality / CAMERA: photo mode's camera controls, and what the
// Gallery and the passport offer for a photo. The engine owns photo mode (`/photo_mode` reports the
// free camera's position, its roll and whether the HUD is hidden); the keys are the engine's
// default bindings in photo mode (crates/common/src/inputs.rs, crates/camera_reel/src/mode.rs):
// W/S/A/D move the camera, R/F raise and lower it, Q/E tilt it, U hides the HUD. The Gallery
// (library/ui/apps/hud/features/gallery) shows each photo as `Open photo`, with a `Photo options`
// button that opens the `Photo options` menu: `Set as Public` (a toggle, `menuitemcheckbox`),
// `Share on X`, `Copy Link`, `Download` and `Delete`; the opened photo (`Photo` dialog) offers
// Download, Copy link, Share and Delete, and its Close (Escape does not close it). A public photo shows in the own passport's PHOTOS tab.
//
// | QA case            | Here                                                                       | Class   |
// | Camera Shortcuts   | in photo mode W moves the camera, R raises and F lowers it, Q tilts it, U hides and shows the HUD | Partial |
// | Photo Properties   | a photo's options menu lists Set as Public, Share on X, Copy Link, Download and Delete; the opened photo offers Download, Copy link, Share and Delete | Partial |
// | Publish Photos     | Set as Public on a new photo, then the own passport's PHOTOS tab shows it  | Port    |
//
// Partial: Camera Shortcuts does not check the drag to move, the wheel's zoom, Shift's speed or N's
// names (no state reports them); Photo Properties lists the actions without running Share, Copy
// Link or Download, which leave the client. Every scenario that takes a photo deletes it again: the
// account's camera reel is shared with later runs and its storage is finite. The Gallery lists the
// newest photo first, so the photo a scenario takes is the first tile.
import assert from 'node:assert/strict'
import { readTarget, spec, until, type Explorer, type Selector } from '../driver/index.ts'
import { closePage, openFromRail, openWithKey, pressRailRow } from './pages.ts'

const scenario = spec(import.meta.url)

const TEXT_TIMEOUT_MS = 10_000
const UPLOAD_TIMEOUT_MS = 60_000
const PUBLISH_TIMEOUT_MS = 30_000
const PASSPORT_TIMEOUT_MS = 40_000
/** How long a camera key is held: long enough for the damped camera to move measurably. */
const HOLD_MS = 700

const SHOOT: Selector = { name: 'Shoot', role: 'button' }
const OPEN_PHOTO: Selector = { name: 'Open photo', role: 'button', index: 0 }
const OPTIONS: Selector = { name: 'Photo options', role: 'button', index: 0 }
const OPTIONS_MENU: Selector = { name: 'Photo options', role: 'menu' }
const SET_PUBLIC: Selector = { name: 'Set as Public', role: 'menuitemcheckbox' }
const MENU_ACTIONS = ['Share on X', 'Copy Link', 'Download', 'Delete']
const VIEWER: Selector = { name: 'Photo', role: 'dialog' }
const VIEWER_ACTIONS = ['Download', 'Copy link', 'Share', 'Delete']
const PASSPORT: Selector = { name: 'Passport sections', role: 'tablist' }
const PHOTOS_TAB: Selector = { name: 'PHOTOS', role: 'tab' }
/** The passport's photos load behind this spinner (PassportPhotos.tsx), with no `aria-busy`. */
const LOADING: Selector = { name: 'Loading', role: 'status' }
const NO_PHOTOS: Selector = { name: 'No photos here yet!', exact: false }

/** The id of `address`'s newest public photo, as the passport reads it: the camera reel's unsigned
 *  list (publicReels.ts), which holds only public photos, newest first. Null when there is none. */
async function newestPublicPhotoId(address: string): Promise<string | null> {
  const url = `https://camera-reel-service.${readTarget().baseDomain}/api/users/${address.toLowerCase()}/images?limit=1&offset=0&compact=true`
  const res = await fetch(url)
  if (!res.ok) throw new Error(`the camera reel answered ${res.status} for the public photos`)
  const body = (await res.json()) as { images?: { id: string }[] }
  return body.images?.[0]?.id ?? null
}

/** The own passport's PHOTOS tab once its photos have loaded (the Loading spinner gone): whether
 *  it shows a first photo tile. The passport is closed again either way. */
async function passportShowsAPhoto(explorer: Explorer): Promise<boolean> {
  await pressRailRow(explorer, { button: 'Me', row: 'Profile' })
  await explorer.hud.wait(PASSPORT, { timeoutMs: PASSPORT_TIMEOUT_MS })
  try {
    await explorer.hud.click(PHOTOS_TAB)
    await explorer.hud.waitHidden(LOADING, PASSPORT_TIMEOUT_MS)
    return (await explorer.hud.isVisible(OPEN_PHOTO)) && !(await explorer.hud.isVisible(NO_PHOTOS))
  } finally {
    await explorer.hud.click({ name: 'Close', role: 'button' })
    await explorer.hud.waitHidden(PASSPORT, 20_000)
  }
}

/** Take a photo and wait for its upload; the restore registered first deletes it whatever happens. */
async function takeUploadedPhoto(explorer: Explorer, restore: (undo: () => Promise<void>) => void): Promise<string> {
  restore(async () => {
    await explorer.photoMode('off')
  })
  await explorer.photoMode('on')
  const before = await explorer.waitPhotoMode(true)
  restore(async () => {
    const { lastPhotoId } = await explorer.photoMode()
    if (lastPhotoId && lastPhotoId !== before.lastPhotoId) await explorer.deletePhoto(lastPhotoId)
  })
  await explorer.hud.wait(SHOOT, { timeoutMs: TEXT_TIMEOUT_MS })
  await explorer.key('Space')
  const settled = await explorer.waitPhotoState((state) => state.uploaded > before.uploaded || state.failed > before.failed, 'the photo upload to settle', UPLOAD_TIMEOUT_MS)
  assert.equal(settled.failed, before.failed, `the upload should succeed; last error: ${settled.lastError}`)
  assert.ok(settled.lastPhotoId, 'the service answers with the photo id')
  await explorer.photoMode('off')
  await explorer.waitPhotoMode(false)
  return settled.lastPhotoId
}

/** Hold a key in photo mode, then let it go. */
async function hold(explorer: Explorer, code: string): Promise<void> {
  await explorer.key(code, 'down')
  await new Promise((resolve) => setTimeout(resolve, HOLD_MS))
  await explorer.key(code, 'up')
}

async function openGalleryWithPhoto(explorer: Explorer): Promise<void> {
  // right after photo mode closes the rail's click can be lost: G opens the page then
  try {
    await openFromRail(explorer, 'gallery')
  } catch {
    await openWithKey(explorer, 'gallery')
  }
  await explorer.hud.wait(OPEN_PHOTO, { state: 'enabled', timeoutMs: PASSPORT_TIMEOUT_MS })
}

/** Close the opened photo with its own Close, which comes after the page's in the document. */
async function closeViewer(explorer: Explorer): Promise<void> {
  const closes = (await explorer.hud.list('button')).filter((button) => button.name === 'Close').length
  await explorer.hud.click({ name: 'Close', role: 'button', index: closes - 1 })
  await explorer.hud.waitHidden(VIEWER, TEXT_TIMEOUT_MS)
}

/** Open the first photo's options menu. The tile shows its options button only while the pointer is
 *  over it or focus is inside it (`display: none` otherwise, ReelGrid.module.css), and the driver's
 *  click moves the pointer and presses in one go: Chrome applies the hover a frame later, so the
 *  press lands on the tile's Open photo and opens the viewer. That viewer is closed with Escape, which
 *  leaves the pointer where it is, over the tile: once the viewer is gone Chrome re-reads the hover,
 *  the options button shows, and a plain click on it opens the menu. */
async function openOptions(explorer: Explorer): Promise<void> {
  for (let attempt = 0; attempt < 3; attempt++) {
    if (await explorer.hud.isVisible(OPTIONS)) {
      await explorer.hud.click(OPTIONS)
    } else {
      // where the options button shows: 12 px in from the tile's right edge, 16 px wide, 33 px down (ReelGrid)
      const tile = await explorer.hud.get(OPEN_PHOTO)
      await explorer.clickAt(tile.x + tile.width - 20, tile.y + 33)
    }
    const opened = await until(
      async () => ((await explorer.hud.isVisible(OPTIONS_MENU)) ? 'menu' : (await explorer.hud.isVisible(VIEWER)) ? 'viewer' : undefined),
      (which) => which !== undefined,
      { timeoutMs: TEXT_TIMEOUT_MS, what: "the photo's options menu" }
    ).catch(() => undefined)
    if (opened === 'menu') return
    if (opened === 'viewer') {
      await explorer.escape()
      const gone = await explorer.hud.waitHidden(VIEWER, TEXT_TIMEOUT_MS).then(() => true, () => false)
      if (!gone) await closeViewer(explorer)
      // the pointer, still over the tile, reveals its options button once the hover is re-read
      await until(() => explorer.hud.isVisible(OPTIONS), (shown) => shown, { timeoutMs: 3_000, intervalMs: 100, what: 'the options button' }).catch(() => undefined)
    }
  }
  assert.fail("the first photo's options menu did not open")
}

scenario('photo mode keys move, raise, lower and tilt the camera, and U hides the HUD', async ({ explorer, restore }) => {
  restore(async () => {
    const state = await explorer.photoMode()
    if (state.active && state.hudHidden) await explorer.key('KeyU')
    await explorer.photoMode('off')
  })
  await explorer.photoMode('on')
  const start = await explorer.waitPhotoMode(true)
  await explorer.hud.wait(SHOOT, { timeoutMs: TEXT_TIMEOUT_MS })
  const distance = (a: [number, number, number], b: [number, number, number]): number => Math.hypot(a[0] - b[0], a[2] - b[2])

  await hold(explorer, 'KeyR')
  const raised = await explorer.waitPhotoState((state) => state.position[1] > start.position[1] + 0.05, 'R to raise the photo camera')
  await hold(explorer, 'KeyF')
  const lowered = await explorer.waitPhotoState((state) => state.position[1] < raised.position[1] - 0.05, 'F to lower the photo camera')

  await hold(explorer, 'KeyW')
  await explorer.waitPhotoState((state) => distance(state.position, lowered.position) > 0.1, 'W to move the photo camera')

  if (start.roll === undefined) assert.fail('/photo_mode reports no roll, so Q and E cannot be checked')
  await hold(explorer, 'KeyQ')
  await explorer.waitPhotoState((state) => Math.abs((state.roll ?? 0) - (start.roll ?? 0)) > 0.5, 'Q to tilt the photo camera')

  const shown = await explorer.photoMode()
  assert.notEqual(shown.hudHidden, true, 'the HUD should show when photo mode opens')
  await explorer.key('KeyU')
  await explorer.waitPhotoState((state) => state.hudHidden === true, 'U to hide the HUD')
  await explorer.key('KeyU')
  await explorer.waitPhotoState((state) => state.hudHidden === false, 'U again to show the HUD')
  await explorer.photoMode('off')
  await explorer.waitPhotoMode(false)
})

scenario("a photo's options menu and its opened view list the photo's actions", async ({ explorer, restore }) => {
  await takeUploadedPhoto(explorer, restore)
  await openGalleryWithPhoto(explorer)

  await openOptions(explorer)
  const toggle = await explorer.hud.get(SET_PUBLIC)
  assert.equal(toggle.checked, false, 'a new photo should not be public')
  const items = (await explorer.hud.list('menuitem')).map((item) => item.name)
  for (const action of MENU_ACTIONS) assert.ok(items.includes(action), `the options menu should offer ${action}: ${JSON.stringify(items)}`)
  await explorer.escape()
  await explorer.hud.waitHidden(OPTIONS_MENU, TEXT_TIMEOUT_MS)

  await explorer.hud.click(OPEN_PHOTO)
  await explorer.hud.wait(VIEWER, { timeoutMs: TEXT_TIMEOUT_MS })
  const buttons = (await explorer.hud.list('button')).map((button) => button.name)
  for (const action of VIEWER_ACTIONS) assert.ok(buttons.includes(action), `the opened photo should offer ${action}: ${JSON.stringify(buttons)}`)
  await closeViewer(explorer)
  await closePage(explorer, 'gallery')
})

scenario('a photo set as public shows in the own passport PHOTOS tab', async ({ explorer, t, restore }) => {
  const address = (await explorer.worldState()).address
  assert.ok(address, 'the player should be signed in')
  const photoId = await takeUploadedPhoto(explorer, restore)
  await openGalleryWithPhoto(explorer)
  await openOptions(explorer)
  assert.equal((await explorer.hud.get(SET_PUBLIC)).checked, false, 'a new photo should not be public')
  await explorer.hud.click(SET_PUBLIC)
  await until(() => explorer.hud.find(SET_PUBLIC), (item) => item === undefined || item.checked === true, { timeoutMs: TEXT_TIMEOUT_MS, what: 'Set as Public to switch on' })
  if (await explorer.hud.isVisible(OPTIONS_MENU)) {
    await explorer.escape()
    await explorer.hud.waitHidden(OPTIONS_MENU, TEXT_TIMEOUT_MS)
  }
  // the menu shows the toggle at once; the list takes the new state once the service has answered,
  // so the menu, opened again, reads it from the list: it is reopened until it does
  await until(
    async () => {
      await openOptions(explorer)
      const checked = (await explorer.hud.get(SET_PUBLIC)).checked
      await explorer.escape()
      await explorer.hud.waitHidden(OPTIONS_MENU, TEXT_TIMEOUT_MS)
      return checked
    },
    (checked) => checked === true,
    { timeoutMs: PUBLISH_TIMEOUT_MS, intervalMs: 1000, what: 'the photo to read as public in its options menu' }
  )
  await closePage(explorer, 'gallery')

  // The passport lists the public photos newest first, from the list read here, and its tiles carry
  // no photo id: this photo heading that list is what puts it on the passport's first tile. Earlier
  // public photos on the shared account (one a failed restore left) cannot satisfy it, and the
  // virtualized grid's tile count does not matter.
  await until(() => newestPublicPhotoId(address), (id) => id === photoId, {
    timeoutMs: PUBLISH_TIMEOUT_MS,
    intervalMs: 1000,
    what: `photo ${photoId} to head the account's public photos`
  })
  assert.ok(await passportShowsAPhoto(explorer), 'the passport PHOTOS tab should show the public photo, not "No photos here yet!"')
  t.diagnostic(`photo ${photoId} is public and first in the passport`)
})
