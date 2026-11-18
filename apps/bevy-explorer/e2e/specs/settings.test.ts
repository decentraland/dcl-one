// QA Automation Suite, General Functionality: SETTINGS (4) and AUTO TRANSLATE (1 of 4). The Settings
// page (library/ui/apps/hud/features/settings/SettingsPanel.tsx) draws the engine's settings
// (system_bridge/src/settings) by category, one tab each, plus the HUD's own chat settings in the
// Chat tab and a Key Bindings tab; every engine setting is a slider (with `decrease`/`increase`
// arrows), a switch (an off/on pair) or a select (a button that opens a listbox), named by the
// setting's title.
//
// | Checklist case          | Here                                                                      | Class   |
// | Graphics                | the Graphics and Performance tabs list the case's settings; an `increase` | Partial |
// |                         | on Ambient Brightness reaches the engine (`/debug_settings Graphics`)     |         |
// | Sound                   | the Audio tab lists Master, Scene, Avatar, Voice and System; a `decrease` | Port    |
// |                         | on Master Volume reaches the engine (`/debug_settings Audio`)             |         |
// | Controls                | the Controls tab lists the sensitivities; an `increase` on Camera         | Partial |
// |                         | Sensitivity reaches the engine (`/debug_settings Controls`)               |         |
// | Chat                    | the Chat tab lists Notification Ping, Receive direct messages from and    | Partial |
// |                         | Language; the Auto-Translate switch and the ping's select change          |         |
// | Translate Option        | the Language select lists the case's ten languages, by their own names    | Port    |
//
// Partial:
// - Graphics: the web build has no Window mode, Fullscreen Resolution or SSAO (the engine leaves them
//   out on wasm32) and Bevy has no quality presets beyond the Graphics Preset button, no memory limit
//   and no dynamic resolution; Render Resolution, Target Frame Rate, the load distances and Max
//   Avatars are in the Performance tab. What "reflected in world" means is read as the engine's
//   value: a change made in the HUD must show in `/debug_settings`, not only in the slider.
// - Controls: there is no separate vertical and horizontal sensitivity; Bevy has Camera Sensitivity,
//   Pointer and Locked Camera sensitivity, Camera Zoom, Scroll and Movement sensitivity, and Camera
//   smoothing. Rebinding a key (the Key Bindings tab) is not exercised.
// - Chat: Bevy has no In-World chat bubbles setting. The chat settings are the HUD's (translation,
//   ping) and the engine's (DM privacy, Language); the Chat category is not in `/debug_settings`,
//   so the HUD's own controls are what is read. DM privacy is not changed: it is stored on the
//   social service, for the shared account.
//
// Every change is put back from the engine's values read before it (`/debug_settings <category>
// <index> <value>`), and the HUD's switches and selects by the HUD, whatever the outcome.
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type Selector } from '../driver/index.ts'
import { PAGES, pressRailRow } from './pages.ts'

const scenario = spec(import.meta.url)

const PAGE_TIMEOUT_MS = 20_000
const CONTROL_TIMEOUT_MS = 10_000
/** A change in the HUD reaches the engine and comes back in the next settings push. */
const APPLY_TIMEOUT_MS = 10_000

const HEADING: Selector = { name: 'Settings', role: 'heading' }
const CATEGORIES: Selector = { name: 'Settings categories', role: 'tablist' }

const slider = (name: string): Selector => ({ name, role: 'slider' })
const select = (name: string): Selector => ({ name, role: 'button' })
const toggle = (name: string): Selector => ({ name, role: 'switch' })

type Values = Record<string, number>

/** The engine's values of a category, by setting index (`/debug_settings <category>`). */
async function engineValues(explorer: Explorer, category: string): Promise<Values> {
  return JSON.parse(await explorer.run(`/debug_settings ${category}`)) as Values
}

/** The indices whose value differs between two reads of a category. */
function changedKeys(before: Values, after: Values): string[] {
  return Object.keys(after).filter((key) => after[key] !== before[key])
}

/** Put a category back as it was read, setting by setting, through the engine. */
async function restoreValues(explorer: Explorer, category: string, before: Values): Promise<void> {
  const now = await engineValues(explorer, category)
  for (const key of changedKeys(before, now)) await explorer.run(`/debug_settings ${category} ${key} ${before[key]}`)
}

/** The Settings page, from the rail, as a user opens it. */
async function openSettings(explorer: Explorer): Promise<void> {
  // the rail's System panel holds the Settings row (#929)
  if (!(await explorer.hud.isVisible(HEADING))) await pressRailRow(explorer, PAGES.settings.rail!)
  await explorer.hud.wait(HEADING, { timeoutMs: PAGE_TIMEOUT_MS })
  await explorer.hud.wait(CATEGORIES, { timeoutMs: CONTROL_TIMEOUT_MS })
}

/** Close the page: Escape, then the top bar's Close should the press not take. */
async function closeSettings(explorer: Explorer): Promise<void> {
  if (!(await explorer.hud.isVisible(HEADING))) return
  await explorer.escape()
  const closed = await explorer.hud.waitHidden(HEADING, 5_000).then(() => true, () => false)
  if (closed && !(await explorer.hud.isVisible(HEADING))) return
  await explorer.hud.click({ name: 'Close', role: 'button', index: 0 })
  await explorer.hud.waitHidden(HEADING, PAGE_TIMEOUT_MS)
}

/** Show a category's tab and wait for one of its controls. */
async function showTab(explorer: Explorer, tab: string, control: Selector): Promise<void> {
  await explorer.hud.click({ name: tab, role: 'tab' })
  await explorer.hud.wait(control, { timeoutMs: CONTROL_TIMEOUT_MS })
}

/** Every named control is on the page: an error naming the ones that are not. */
async function assertControls(explorer: Explorer, controls: Selector[], where: string): Promise<void> {
  const missing: string[] = []
  for (const control of controls) if (!(await explorer.hud.isVisible(control))) missing.push(JSON.stringify(control))
  assert.deepEqual(missing, [], `the ${where} should show every one of its settings`)
}

/** Press a slider's own arrow. The arrows are named alike for every slider; each sits right after
 *  (`increase`) or right before (`decrease`) its slider, so the slider's place among the page's
 *  sliders is the arrow's index. */
async function stepSlider(explorer: Explorer, name: string, arrow: 'increase' | 'decrease'): Promise<void> {
  const sliders = (await explorer.hud.list('slider')).map((entry) => entry.name)
  const index = sliders.findIndex((entry) => entry.toLowerCase() === name.toLowerCase())
  assert.ok(index >= 0, `the page should show the ${name} slider (it shows ${sliders.join(', ')})`)
  await explorer.hud.click({ name: arrow, role: 'button', index })
}

/** A slider's arrow changes the slider and exactly one engine value of its category; the step is
 *  put back through the engine. */
async function stepReachesEngine(explorer: Explorer, restore: (undo: () => Promise<void>) => void, category: string, name: string, arrow: 'increase' | 'decrease'): Promise<void> {
  const before = await engineValues(explorer, category)
  restore(() => restoreValues(explorer, category, before))
  const shown = Number(await explorer.hud.value(slider(name)))
  await stepSlider(explorer, name, arrow)
  const now = await until(
    () => explorer.hud.value(slider(name)).then(Number),
    (value) => value !== shown,
    { timeoutMs: APPLY_TIMEOUT_MS, what: `the ${name} slider to move from ${shown} after its ${arrow} arrow` }
  )
  assert.ok(arrow === 'increase' ? now > shown : now < shown, `${name} should ${arrow}: it went from ${shown} to ${now}`)
  const after = await until(
    () => engineValues(explorer, category),
    (values) => changedKeys(before, values).length > 0,
    { timeoutMs: APPLY_TIMEOUT_MS, what: `the engine's ${category} settings to take the new ${name}` }
  )
  const changed = changedKeys(before, after)
  assert.equal(changed.length, 1, `one ${category} setting should change, ${name}: ${changed.join(', ')} did`)
  assert.equal(after[changed[0]!], now, `the engine's ${name} should be the slider's ${now}`)
}

scenario('the Graphics and Performance tabs list the graphics settings, and a change reaches the engine', async ({ explorer, restore }) => {
  restore(() => closeSettings(explorer))
  await openSettings(explorer)
  await showTab(explorer, 'Graphics', slider('Ambient Brightness'))
  await assertControls(explorer, [
    slider('Shadow Distance'),
    slider('Shadow Caster Count'),
    select('Shadow settings'),
    select('Anti-aliasing'),
    select('Bloom'),
    select('Fog'),
    select('Depth of Field'),
    toggle('Avatar Cel Shading'),
    toggle('Avatar Outline'),
    toggle('Avatar Ghosts'),
    select('Distant Scene Rendering')
  ], 'Graphics tab')
  await stepReachesEngine(explorer, restore, 'Graphics', 'Ambient Brightness', 'increase')
  await showTab(explorer, 'Performance', slider('Render Resolution'))
  await assertControls(explorer, [
    select('Target Frame Rate'),
    slider('Scene Load Distance'),
    slider('Scene Unload Distance'),
    slider('Max Avatars')
  ], 'Performance tab')
})

scenario('the Audio tab lists the volume channels, and Master Volume reaches the engine', async ({ explorer, restore }) => {
  restore(() => closeSettings(explorer))
  await openSettings(explorer)
  await showTab(explorer, 'Audio', slider('Master Volume'))
  await assertControls(explorer, ['Scene Volume', 'Avatar Volume', 'Voice Volume', 'System Volume', 'Voice Chat Volume'].map(slider), 'Audio tab')
  await stepReachesEngine(explorer, restore, 'Audio', 'Master Volume', 'decrease')
})

scenario('the Controls tab lists the sensitivities, and Camera Sensitivity reaches the engine', async ({ explorer, restore }) => {
  restore(() => closeSettings(explorer))
  await openSettings(explorer)
  await showTab(explorer, 'Controls', slider('Camera Sensitivity'))
  await assertControls(explorer, [
    slider('Pointer and Locked Camera sensitivity'),
    slider('Camera Zoom'),
    slider('Scroll sensitivity'),
    slider('Movement sensitivity'),
    select('Camera smoothing')
  ], 'Controls tab')
  await stepReachesEngine(explorer, restore, 'Controls', 'Camera Sensitivity', 'increase')
})

/** Open a select and choose an option by name; the button then reads it. */
async function choose(explorer: Explorer, name: string, option: string): Promise<void> {
  await explorer.hud.click(select(name))
  await explorer.hud.click({ name: option, role: 'option' })
  await explorer.hud.waitHidden({ name: option, role: 'option' }, CONTROL_TIMEOUT_MS)
  await until(() => explorer.hud.text(select(name)), (text) => text.includes(option), { timeoutMs: APPLY_TIMEOUT_MS, what: `${name} to read ${option}` })
}

scenario('the Chat tab lists the chat settings, and its switch and ping change', async ({ explorer, restore }) => {
  restore(() => closeSettings(explorer))
  await openSettings(explorer)
  await showTab(explorer, 'Chat', select('Notification Ping'))
  await assertControls(explorer, [select('Receive direct messages from'), select('Language'), toggle('Auto-Translate New Conversations')], 'Chat tab')

  const auto = toggle('Auto-Translate New Conversations')
  const wasOn = (await explorer.hud.get(auto)).checked === true
  restore(async () => {
    await openSettings(explorer)
    await showTab(explorer, 'Chat', auto)
    if ((await explorer.hud.get(auto)).checked !== wasOn) await explorer.hud.click(auto)
  })
  await explorer.hud.click(auto)
  await until(() => explorer.hud.get(auto), (control) => control.checked === !wasOn, { timeoutMs: APPLY_TIMEOUT_MS, what: 'the Auto-Translate switch to flip' })

  const ping = select('Notification Ping')
  const shown = (await explorer.hud.text(ping)).trim()
  await explorer.hud.click(ping)
  const options = (await explorer.hud.list('option')).map((entry) => entry.name)
  assert.ok(options.length >= 2, `the ping should offer a choice: ${options.join(', ')}`)
  const other = options.find((option) => !shown.includes(option))!
  await explorer.hud.click({ name: other, role: 'option' })
  restore(async () => {
    await openSettings(explorer)
    await showTab(explorer, 'Chat', ping)
    const original = options.find((option) => shown.includes(option))
    if (original && !(await explorer.hud.text(ping)).includes(original)) await choose(explorer, 'Notification Ping', original)
  })
  await until(() => explorer.hud.text(ping), (text) => text.includes(other), { timeoutMs: APPLY_TIMEOUT_MS, what: `the ping to read ${other}` })
})

/** The case's ten languages, by the names the select gives them (each its own). */
const CASE_LANGUAGES: Record<string, string[]> = {
  English: ['English'],
  Spanish: ['Español'],
  French: ['Français'],
  German: ['Deutsch'],
  Russian: ['Русский'],
  Portuguese: ['Português', 'Português (Brasil)'],
  Italian: ['Italiano'],
  Chinese: ['简体中文', '繁體中文'],
  Japanese: ['日本語'],
  Korean: ['한국어']
}

scenario('the Language select lists the languages messages translate to', async ({ explorer, restore }) => {
  restore(() => closeSettings(explorer))
  await openSettings(explorer)
  await showTab(explorer, 'Chat', select('Language'))
  const current = (await explorer.hud.text(select('Language'))).trim()
  await explorer.hud.click(select('Language'))
  const options = (await explorer.hud.list('option')).map((entry) => entry.name)
  // close the list without a change: the option the select shows, chosen again
  const selected = options.find((option) => option === current) ?? options.find((option) => current.includes(option)) ?? options[0]!
  const missing = Object.entries(CASE_LANGUAGES).filter(([, names]) => !names.some((name) => options.includes(name))).map(([language]) => language)
  await explorer.hud.click({ name: selected, role: 'option', index: 0 })
  assert.deepEqual(missing, [], `the Language select should offer every case language (it offers ${options.length}: ${options.join(', ')})`)
  assert.ok(options.some((option) => option.startsWith('System')), 'the first option should follow the system language')
})
