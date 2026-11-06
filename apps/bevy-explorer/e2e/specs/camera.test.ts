// explorer-automation `CameraTests` (3): photo mode (C or the sidebar's camera button) and the
// capture. The engine owns photo mode (`/photo_mode`, `/take_photo`); the HUD shows its overlay
// (the frame, Shoot, Gallery, Close) while it is on.
//
// | Unity test                    | Here                                                    | Class   |
// | TestToggleCameraWithShortcut  | C opens photo mode, C again closes it                   | Port    |
// | TestOpenCameraFromSidebar     | the rail's Camera button opens it; its Close closes it  | Port    |
// | TestTakePhoto                 | Space captures and uploads; the counters, the photo id  | Port    |
import assert from 'node:assert/strict'
import { spec } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const SHOOT = { name: 'Shoot', role: 'button' } as const
const PAGE_TIMEOUT_MS = 10_000

// Every scenario restores photo mode off: while it is on the engine reserves all input, which
// would hold a following scenario that skips the opening Escape.
scenario('C toggles photo mode', async ({ explorer, restore }) => {
  restore(async () => {
    await explorer.photoMode('off')
  })
  await explorer.key('KeyC')
  await explorer.waitPhotoMode(true)
  await explorer.hud.wait(SHOOT, { timeoutMs: PAGE_TIMEOUT_MS })
  await explorer.key('KeyC')
  await explorer.waitPhotoMode(false)
  assert.equal(await explorer.hud.isVisible(SHOOT), false, 'the photo overlay should close with photo mode')
})

scenario('the rail opens photo mode', async ({ explorer, restore }) => {
  restore(async () => {
    await explorer.photoMode('off')
  })
  await explorer.hud.click({ name: 'Camera', role: 'button' })
  await explorer.waitPhotoMode(true)
  await explorer.hud.wait(SHOOT, { timeoutMs: PAGE_TIMEOUT_MS })
  await explorer.hud.click({ name: 'Close camera', role: 'button' })
  await explorer.waitPhotoMode(false)
})

// The upload goes to the shared test account's camera reel, whose storage is finite: the restore
// deletes the photo so the scenario does not fill it over time. It is registered before the shot
// and reads the photo id itself, so a slow upload or a failed assertion still deletes the photo.
scenario('Space in photo mode takes a photo', async ({ explorer, restore }) => {
  restore(async () => {
    await explorer.photoMode('off')
  })
  await explorer.photoMode('on')
  const before = await explorer.waitPhotoMode(true)
  restore(async () => {
    const { lastPhotoId } = await explorer.photoMode()
    if (lastPhotoId && lastPhotoId !== before.lastPhotoId) await explorer.deletePhoto(lastPhotoId)
  })
  await explorer.hud.wait(SHOOT, { state: 'actionable', timeoutMs: PAGE_TIMEOUT_MS })
  await explorer.key('Space')
  const captured = await explorer.waitPhotoState((state) => state.captures > before.captures, 'the capture to start')
  assert.equal(captured.active, true, 'photo mode stays on after a photo')
  const settled = await explorer.waitPhotoState(
    (state) => state.uploaded > before.uploaded || state.failed > before.failed,
    'the upload to settle',
    60_000
  )
  assert.equal(settled.failed, before.failed, `the upload should succeed; last error: ${settled.lastError}`)
  assert.ok(settled.lastPhotoId, 'the service answers with the photo id')
  await explorer.photoMode('off')
  await explorer.waitPhotoMode(false)
})
