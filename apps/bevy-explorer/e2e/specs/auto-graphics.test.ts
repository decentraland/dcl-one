// The Auto Graphics message: the HUD's banner for graphics the engine lowered to keep the frame
// rate. `/auto_graphics_notice` raises a sample message (shadows off, 70%) without changing any
// setting, so the banner is checked on a machine that is not slow.
//
// | Scenario                                              | Class |
// | the message shows what changed and closes with OK     | New   |
import assert from 'node:assert/strict'
import { spec, type Selector } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const BANNER: Selector = { name: 'Graphics lowered', role: 'status' }
const OK: Selector = { name: 'OK', role: 'button' }
const SETTINGS: Selector = { name: 'Settings', role: 'button' }
const TIMEOUT_MS = 20_000

scenario('the message shows what changed and closes with OK', async ({ explorer }) => {
  await explorer.run('/auto_graphics_notice')
  await explorer.hud.wait(BANNER, { timeoutMs: TIMEOUT_MS })
  const text = await explorer.hud.text(BANNER)
  assert.match(text, /Graphics lowered to keep things smooth/)
  assert.match(text, /Shadows off · Resolution 70%/)
  assert.doesNotMatch(text, /Bloom off/)
  assert.ok(await explorer.hud.isVisible(SETTINGS))
  await explorer.hud.wait(OK, { state: 'actionable', timeoutMs: TIMEOUT_MS })
  await explorer.hud.click(OK)
  await explorer.hud.waitHidden(BANNER, TIMEOUT_MS)
})
