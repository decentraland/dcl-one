// `scenario`: one end-to-end test. It shares the process's client, starts with every panel closed
// (Escape, as explorer-automation's SetUp does, until the HUD is at rest), records every console
// command and keeps the final frame, whatever the outcome, and a failed one's page as it looked.
// A scenario the client cannot run is a `todo` with its reason: the report lists it, and it is
// never a pass.
import { after, test, type TestContext } from 'node:test'
import path from 'node:path'
import { type Console, ConsoleError, scrubCredentials } from './console.ts'
import type { Explorer, HudRead } from './explorer.ts'
import { ScenarioReport } from './report.ts'
import type { Selector } from './selectors.ts'
import { BOOT_ALLOWANCE_MS, closeSessions, openExplorer, WORLD_HUD, type OpenOptions } from './session.ts'
import { TimeoutError, until } from './wait.ts'

// Every spec imports this module before its first test, so this hook is the root's: it runs once
// every spec file is done (`--test-isolation=none` keeps them in one process) and closes the
// clients and the web server. Registered from inside a scenario it would be that scenario's, and
// the shared client would close after the first scenario.
after(() => closeSessions())

export interface ScenarioContext {
  explorer: Explorer
  t: TestContext
  report: ScenarioReport
  /** Register how to put back account state the scenario changes (the account is shared with
   *  every later scenario and run). The undos run after the body, whatever its outcome, once the
   *  final frame is kept, last registered first; register one before the change it undoes. */
  restore(undo: () => Promise<void>): void
}

export interface ScenarioOptions {
  /** The reason the client cannot run this scenario yet; the body does not run. */
  todo?: string
  /** A reason to skip; prefer `todo`, which the report counts apart. */
  skip?: string
  /** The body's budget, and each undo's; the first scenario's boot has its own (`BOOT_ALLOWANCE_MS`). */
  timeoutMs?: number
  /** Which client: the default session unless given. */
  session?: OpenOptions
  /** Keep the HUD as the scenario left it (no Escape first). */
  noEscape?: boolean
}

export const DEFAULT_SCENARIO_TIMEOUT_MS = 180_000

/** How many of the page's last console lines a failed scenario keeps. */
export const CONSOLE_TAIL_LINES = 200
/** How long the page's screenshot may take: a page that stopped answering must not hold the run. */
export const EVIDENCE_TIMEOUT_MS = 30_000

/** What a failed scenario keeps beside its command log and the engine's frame, where the transport
 *  can tell: the page as it looks, the HUD included (`page.jpg`: on the web the engine's frame has
 *  no HUD), and the page's last console lines (`console.txt`). */
export async function failureEvidence(console_: Console, timeoutMs = EVIDENCE_TIMEOUT_MS): Promise<Record<string, Uint8Array | string>> {
  if (!console_.evidence) return {}
  const evidence = await withinBudget(console_.evidence(), timeoutMs, "the page's screenshot and console")
  // the report is a public artifact: a line can carry a credential (a room's access token)
  const lines = evidence.console.slice(-CONSOLE_TAIL_LINES).map(scrubCredentials)
  const files: Record<string, Uint8Array | string> = { 'console.txt': `${lines.join('\n')}\n` }
  if (evidence.screenshot) files['page.jpg'] = evidence.screenshot
  return files
}

/** The rail buttons of the panels that open beside the rail, which leave it uncovered: each reads
 *  pressed while its panel is open. (The rail's top button, `WORLD_HUD`, takes a click only with
 *  nothing over the world: a page hides the rail, a popup such as the passport covers it.) */
export const RAIL_PANELS = ['Profile', 'Notifications', 'Skybox', 'Emotes', 'Friends'] as const
/** The rail's own settings popover, which its button shows through `aria-expanded`, not as pressed. */
export const RAIL_SETTINGS: Selector = { name: 'Sidebar settings', role: 'dialog' }
/** The chat input while it has the focus, and with it the keys, which then never reach the engine. */
export const CHAT_TYPING: Selector = { name: 'Write a message', role: 'textbox' }
/** What a client without the HUD answers a `/hud_*` command: the standalone built without the
 *  CEF HUD has none, and one whose page is not up says so (`src/react_hud_cef/automation.rs`). */
const NO_HUD = /Command not recognized|there is no HUD page|the HUD page has not started yet/
/** Escape closes one layer a press (a dropdown, a modal, the page under it, the chat input's
 *  focus), and a failed scenario can leave several. */
export const REST_ESCAPES = 4
/** The most one press is given to close a layer the HUD shows. A press that closes one is followed
 *  as soon as the HUD shows it (the next press waits on that, not on this); this only bounds a
 *  press that closes nothing the HUD reports, which is then followed by the next press anyway. */
export const REST_PRESS_TIMEOUT_MS = 10_000
/** The roles of the layers inside a page that Escape closes: a modal, a dropdown's options. */
const LAYER_ROLES = new Set(['dialog', 'alertdialog', 'option', 'menu', 'listbox'])

/** The HUD as a scenario's start finds it: at `rest`, with something `open` that Escape closes,
 *  or with `no-hud` to read (the standalone built without its bundle). Any other error is the
 *  scenario's: it fails at its start saying so. */
type Rest = 'rest' | 'open' | 'no-hud'

/** The HUD's state as Escape changes it. */
interface Layers {
  rest: Rest
  /** What is open, layer by layer: a press that closed one changes it. */
  open: string
  /** The rail is on its way back in: what the last press closed is still going away. */
  moving: boolean
  /** The chat input has the focus, as the HUD shows it. */
  typing: boolean
  /** The engine's word on the keyboard (`/world_state`): it hears of a field's focus a few
   *  frames after the HUD shows it. */
  keyboardClaimed: boolean | null | undefined
}

const NO_LAYERS: Layers = { rest: 'no-hud', open: '', moving: false, typing: false, keyboardClaimed: null }

/** The HUD has left the chat input but the engine still holds the keys for it: the bridge has
 *  not relayed the blur yet, and a key sent now is typed into nothing. */
function relaying(before: Layers, now: Layers): boolean {
  return before.typing && !now.typing && now.keyboardClaimed === true
}

/** The rail, the panels beside it, the chat input, the layers inside a page, and the engine's
 *  word on whether a field holds the keyboard. At rest the rail takes a click, no panel beside it
 *  is open and no field holds the keys (the chat comes up with its input focused). */
async function hudLayers(explorer: Explorer): Promise<Layers> {
  let rail: HudRead
  try {
    rail = await explorer.hud.read(WORLD_HUD)
  } catch (error) {
    if (error instanceof ConsoleError && NO_HUD.test(error.output)) return NO_LAYERS
    throw error
  }
  const shown = await explorer.hud.list()
  if (!rail.found && shown.length === 0) return NO_LAYERS
  const pressed: string[] = []
  if (rail.found) {
    for (const name of RAIL_PANELS) {
      if ((await explorer.hud.find({ name, role: 'button' }))?.checked) pressed.push(name)
    }
  }
  const typing = await explorer.hud.isVisible(CHAT_TYPING)
  const settings = await explorer.hud.isVisible(RAIL_SETTINGS)
  const { keyboardClaimed } = await explorer.worldState()
  const railState = rail.found ? (rail.actionable ? 'rail' : `rail ${rail.reason ?? 'not actionable'}`) : 'no rail'
  const layers = shown.filter((entry) => LAYER_ROLES.has(entry.role)).map((entry) => `${entry.role} ${entry.name}`)
  const atRest = rail.found && rail.actionable && !typing && !settings && pressed.length === 0 && keyboardClaimed !== true
  return {
    rest: atRest ? 'rest' : 'open',
    open: JSON.stringify({ railState, pressed, typing, settings, layers, keyboardClaimed }),
    moving: rail.found && rail.reason === 'still moving',
    typing,
    keyboardClaimed
  }
}

/** The layers once the rail holds still: a rail still sliding in reads as open with nothing
 *  open. Past `timeoutMs` the last read stands. */
async function stillLayers(explorer: Explorer, timeoutMs: number): Promise<Layers> {
  try {
    return await until(() => hudLayers(explorer), (now) => !now.moving, { timeoutMs, intervalMs: 100, what: 'the HUD rail to hold still' })
  } catch (error) {
    if (!(error instanceof TimeoutError)) throw error
    return hudLayers(explorer)
  }
}

/** Close what an earlier scenario left open, a layer per Escape, until the HUD is at rest. Each
 *  press waits for the HUD to show it took effect (a layer gone, the rail done moving back in,
 *  the engine told of a field the HUD left) before the next, so no press lands in the middle of
 *  the last one's change. A HUD at rest, the rail held still, gets no Escape, which would have
 *  nothing to close. At rest no field holds the keys. A HUD that never gets to rest is left to
 *  the scenario, whose first step then fails saying what it found. */
export async function closeToRest(explorer: Explorer, pressTimeoutMs = REST_PRESS_TIMEOUT_MS): Promise<void> {
  let layers = await stillLayers(explorer, pressTimeoutMs)
  for (let press = 0; press < REST_ESCAPES && layers.rest === 'open'; press++) {
    const before = layers
    await explorer.escape()
    try {
      const took = (now: Layers): boolean => now.rest !== 'open' || (now.open !== before.open && !now.moving && !relaying(before, now))
      layers = await until(() => hudLayers(explorer), took, {
        timeoutMs: pressTimeoutMs,
        intervalMs: 100,
        what: 'the Escape to close a layer of the HUD'
      })
    } catch (error) {
      if (!(error instanceof TimeoutError)) throw error
    }
  }
}

/** The spec file's name for the report, from its `import.meta.url`. */
export function specName(importMetaUrl: string): string {
  return path.basename(new URL(importMetaUrl).pathname).replace(/\.test\.ts$/, '')
}

/** `work`, or a `TimeoutError` once `timeoutMs` have passed. The work is not cancelled: a
 *  scenario's final frame is taken while its body may still be running. */
export function withinBudget<T>(work: Promise<T>, timeoutMs: number, what: string): Promise<T> {
  let timer: NodeJS.Timeout | undefined
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new TimeoutError(what, timeoutMs, undefined)), timeoutMs)
  })
  return Promise.race([work, deadline]).finally(() => clearTimeout(timer))
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function scenario(file: string, name: string, body: (ctx: ScenarioContext) => Promise<void>, options: ScenarioOptions = {}): void {
  if (options.todo) {
    test(name, { todo: options.todo }, () => {})
    return
  }
  if (options.skip) {
    test(name, { skip: options.skip }, () => {})
    return
  }
  const timeoutMs = options.timeoutMs ?? DEFAULT_SCENARIO_TIMEOUT_MS
  // The first scenario of a session boots the client, whose waits bound it; the scenario's own
  // budget starts once it is up. Node's timeout is the sum, and the undos' backstop.
  test(name, { timeout: BOOT_ALLOWANCE_MS + 2 * timeoutMs }, async (t) => {
    const explorer = await openExplorer(options.session)
    const report = new ScenarioReport(file, name)
    const stop = explorer.onCommand((record) => report.record(record))
    const files: Record<string, Uint8Array | string> = {}
    const undos: (() => Promise<void>)[] = []
    const failures: unknown[] = []
    try {
      if (!options.noEscape) await closeToRest(explorer)
      await withinBudget(body({ explorer, t, report, restore: (undo) => undos.push(undo) }), timeoutMs, `the scenario "${name}" to finish`)
    } catch (error) {
      failures.push(error)
    }
    try {
      files['final.png'] = await explorer.screenshot()
    } catch (error) {
      files['final.txt'] = `no final screenshot: ${message(error)}`
    }
    if (failures.length > 0) {
      try {
        Object.assign(files, await failureEvidence(explorer.console))
      } catch (error) {
        files['page.txt'] = `no page evidence: ${message(error)}`
      }
    }
    for (const undo of undos.reverse()) {
      try {
        await withinBudget(undo(), timeoutMs, `the restore after "${name}" to finish`)
      } catch (error) {
        failures.push(error)
      }
    }
    stop()
    await report.write(files)
    t.diagnostic(`${report.commandCount} console commands; report in ${report.dir}`)
    if (failures.length === 1) throw failures[0]
    if (failures.length > 1) throw new AggregateError(failures, failures.map(message).join('; and the restore failed: '))
  })
}

/** A `scenario` bound to one spec file. */
export function spec(importMetaUrl: string): (name: string, body: (ctx: ScenarioContext) => Promise<void>, options?: ScenarioOptions) => void {
  const file = specName(importMetaUrl)
  return (name, body, options) => scenario(file, name, body, options)
}
