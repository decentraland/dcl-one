// explorer-automation `SkyboxTests` (2): the sidebar's Skybox widget.
//
// | Unity test                 | Here                                                              | Class   |
// | TestSkyboxWidgetPresence   | the Skybox panel shows its title, switch, slider and an HH:mm time | Partial |
// | TestModifySkyboxTime       | the engine's /time sets 18:00 and the panel shows it               | Partial |
//
// Partial:
// - TestSkyboxWidgetPresence expects the title "Night/day"; the panel's title is "Skybox" (its
//   dialog name and the start of its text). The `Time progression` switch and the time slider have
//   no accessible name (hud-accessible-names, "Controls without a name") and the time label is a
//   span with no role, so the switch and slider are addressed by role alone (each is the only one
//   on screen with the panel open) and the time is read from the panel's text, which reads
//   `Skybox HH:mm Time progression`.
// - TestModifySkyboxTime writes the slider's value; the slider has no name and where a click lands
//   on it depends on its thumb, so the engine's `/time 18 0` stops the clock at 18:00 instead (the
//   same 0.75 of the day). The panel reads the clock when it opens and does not follow a stopped
//   one, so it is closed and reopened to show it, as Unity reopens its widget after the write. The
//   assertions are Unity's: the label is an HH:mm time within 17:50 to 18:10, and the Time
//   progression switch reaches the state asked of it at each change. Added: the slider reads about
//   18 and is enabled. Unity restores the switch only; here the clock is put back as found too.
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type Selector } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const TEXT_TIMEOUT_MS = 10_000

const SKYBOX_BUTTON: Selector = { name: 'Skybox', role: 'button' }
const PANEL: Selector = { name: 'Skybox', role: 'dialog' }
const AUTO: Selector = { role: 'switch' }
const SLIDER: Selector = { role: 'slider' }

const TARGET_HOURS = 18
const TOLERANCE_MINUTES = 10

/** Click once the control is one a user could click, as explorer-automation's Click waits for it. */
async function press(explorer: Explorer, selector: Selector): Promise<void> {
  await explorer.hud.wait(selector, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.click(selector)
}

async function openSkybox(explorer: Explorer): Promise<void> {
  await press(explorer, SKYBOX_BUTTON)
  await explorer.hud.wait(PANEL, { timeoutMs: CONTROL_TIMEOUT_MS })
}

async function closeSkybox(explorer: Explorer): Promise<void> {
  await explorer.escape()
  await explorer.hud.waitHidden(PANEL, CONTROL_TIMEOUT_MS)
}

/** The time the panel shows, in minutes after midnight; undefined when its text holds no HH:mm. */
async function shownMinutes(explorer: Explorer): Promise<number | undefined> {
  const match = /\b([01]\d|2[0-3]):([0-5]\d)\b/.exec(await explorer.hud.text(PANEL))
  return match ? Number(match[1]) * 60 + Number(match[2]) : undefined
}

/** The engine's sky clock: the hour (to the minute) and the speed, 0 when it is stopped. */
async function readClock(explorer: Explorer): Promise<{ hours: number; speed: number }> {
  const { after, speed } = await explorer.time()
  const [hours, minutes] = after.split(':').map(Number)
  return { hours: hours! + minutes! / 60, speed }
}

/** Wait for the Time progression switch to read `on`. */
async function waitAuto(explorer: Explorer, on: boolean): Promise<void> {
  await until(() => explorer.hud.get(AUTO), (control) => control.checked === on, {
    timeoutMs: TEXT_TIMEOUT_MS,
    what: `the Time progression switch to be ${on ? 'on' : 'off'}`
  })
}

/** Wait for the engine's clock to run (`running`) or to be stopped. */
async function waitClock(explorer: Explorer, running: boolean): Promise<void> {
  await until(() => readClock(explorer), (clock) => (clock.speed !== 0) === running, {
    timeoutMs: TEXT_TIMEOUT_MS,
    what: `the engine's clock to be ${running ? 'running' : 'stopped'}`
  })
}

scenario('the Skybox panel shows its title, the switch, the slider and a time', async ({ explorer }) => {
  await openSkybox(explorer)
  const panel = await explorer.hud.get(PANEL)
  assert.ok(panel.text.startsWith('Skybox'), `the panel should be titled Skybox: ${panel.text}`)
  assert.ok(await explorer.hud.isVisible(AUTO), 'the panel should have the Time progression switch')
  assert.ok(await explorer.hud.isVisible(SLIDER), 'the panel should have the time slider')
  assert.match(panel.text, /\b\d{2}:\d{2}\b/, `the Skybox panel should show a HH:mm time of day: ${panel.text}`)
  await closeSkybox(explorer)
})

scenario('with the day cycle off, the Skybox panel shows the time the clock was set to', async ({ explorer }) => {
  const original = await readClock(explorer)
  const autoWasOn = original.speed !== 0
  try {
    await openSkybox(explorer)
    // The panel reads the clock as it opens and then sets its switch from it. A `/time` of ours is
    // answered after that read, so the press below cannot be overwritten by it.
    await readClock(explorer)
    await waitAuto(explorer, autoWasOn)
    if (autoWasOn) {
      await press(explorer, AUTO)
      await waitAuto(explorer, false)
      // the panel freezes the clock after it reads it: set the time only once that has landed
      await waitClock(explorer, false)
    }
    await closeSkybox(explorer)

    await explorer.time(TARGET_HOURS, 0)
    await openSkybox(explorer)
    await waitAuto(explorer, false)
    await until(() => shownMinutes(explorer), (minutes) => minutes !== undefined && Math.abs(minutes - TARGET_HOURS * 60) <= TOLERANCE_MINUTES, {
      timeoutMs: TEXT_TIMEOUT_MS,
      what: 'the Skybox time label to read an HH:mm time within 10 minutes of 18:00'
    })
    const slider = await explorer.hud.get(SLIDER)
    assert.equal(slider.disabled, false, 'the slider should react while the day cycle is off')
    assert.ok(Math.abs(Number(slider.value) - TARGET_HOURS) <= TOLERANCE_MINUTES / 60, `the slider should read about ${TARGET_HOURS}: ${slider.value}`)

    if (autoWasOn) {
      await press(explorer, AUTO)
      await waitAuto(explorer, true)
      await waitClock(explorer, true)
    }
    await closeSkybox(explorer)
  } finally {
    await explorer.time(original.hours, original.speed)
  }
})
