// The Auto Graphics message: the HUD's toast for graphics the engine lowered to keep the frame
// rate. `/auto_graphics_notice` raises a sample message (shadows off, 70%) without changing any
// setting, so the toast is checked on a machine that is not slow.
//
// Auto Graphics is turned off for the run, so a real notice from a slow machine cannot interleave
// with the sample one, and is turned back on afterwards. The toast is found by its Settings button
// (`Open graphics settings`): a toast is a `status` with no name of its own. Its words are pinned
// by the HUD's tests (`autoGraphics.test.tsx`).
//
// | Scenario                                                       | Class |
// | the message offers Settings, which opens the Performance tab   | New   |
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type Selector } from '../driver/index.ts'
import { closePage, openWithKey } from './pages.ts'

const scenario = spec(import.meta.url)

const TOAST_SETTINGS: Selector = { name: 'Open graphics settings', role: 'button' }
const PERFORMANCE_TAB: Selector = { name: 'Performance', role: 'tab' }
const AUTO_GRAPHICS: Selector = { name: 'Auto Graphics', role: 'switch' }
const TIMEOUT_MS = 20_000
/** How long one sample message is given to reach the toast before it is raised again: the relay
 *  does not replay a message it dropped while backing off. */
const NOTICE_WAIT_MS = 3_000

interface AutoGraphicsState {
  enabled: boolean
}

const enabled = async (explorer: Explorer): Promise<boolean> => (await explorer.json<AutoGraphicsState>('/auto_graphics_state')).enabled

/** Set the Auto Graphics switch on the Settings page's Performance tab, and close the page. */
async function setAutoGraphics(explorer: Explorer, on: boolean): Promise<void> {
  await openWithKey(explorer, 'settings')
  await explorer.hud.click(PERFORMANCE_TAB)
  const control = await explorer.hud.waitActionable(AUTO_GRAPHICS, TIMEOUT_MS)
  if (control.checked !== on) await explorer.hud.click(AUTO_GRAPHICS)
  await until(() => enabled(explorer), (now) => now === on, { timeoutMs: TIMEOUT_MS, what: `Auto Graphics to be ${on ? 'on' : 'off'}` })
  await closePage(explorer, 'settings')
}

scenario('the message offers Settings, which opens the Performance tab', async ({ explorer, restore }) => {
  const wasOn = await enabled(explorer)
  restore(async () => {
    await closePage(explorer, 'settings')
    if (wasOn && !(await enabled(explorer))) await setAutoGraphics(explorer, true)
  })
  if (wasOn) await setAutoGraphics(explorer, false)
  // a message from before the switch goes by itself, within the toast's 15 s
  await explorer.hud.waitHidden(TOAST_SETTINGS, TIMEOUT_MS)

  await until(
    async () => {
      await explorer.run('/auto_graphics_notice')
      try {
        await explorer.hud.wait(TOAST_SETTINGS, { timeoutMs: NOTICE_WAIT_MS })
        return true
      } catch {
        return false
      }
    },
    (shown) => shown,
    { timeoutMs: 3 * TIMEOUT_MS, intervalMs: 0, what: 'the Auto Graphics toast to show' }
  )

  await explorer.hud.waitActionable(TOAST_SETTINGS, TIMEOUT_MS)
  await explorer.hud.click(TOAST_SETTINGS)
  await explorer.hud.waitHidden(TOAST_SETTINGS, TIMEOUT_MS)
  // the page opened on Performance, where the setting is, not on the tab last used
  await explorer.hud.wait(AUTO_GRAPHICS, { timeoutMs: TIMEOUT_MS })
  assert.ok(await explorer.hud.isVisible(PERFORMANCE_TAB), 'the Performance tab should be on the Settings page')
})
