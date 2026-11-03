// The typed API specs use. Every method is one console command (or a command and the wait that
// makes its effect observable), with the reply parsed as rendering/bevy-explorer/docs/guidelines/
// running.md ("Test commands") describes it. Specs never see which transport carries the line.
import { type Console, ConsoleError, redact, redactLine } from './console.ts'
import { type AuthIdentity, encodeIdentity } from './identity.ts'
import { type Selector, describeSelector, selectorArg, toSelectorObject } from './selectors.ts'
import { until } from './wait.ts'

export interface WorldState {
  inWorld: boolean
  arrivals: number
  signedIn: boolean
  address: string | null
  guest: boolean
  realm: string
  realmConnected: boolean
  worldHeld: boolean
  outOfWorld: boolean | null
  position: [number, number, number] | null
}

export interface Control {
  found: true
  role: string
  name: string
  text: string
  value?: string
  checked?: boolean
  disabled: boolean
  visible: boolean
  actionable: boolean
  reason?: string
  x: number
  y: number
  width: number
  height: number
}

export interface NotFound {
  found: false
  error?: string
}

export type HudRead = Control | NotFound

export type InputKind = 'trusted' | 'synthetic'

export interface HudInput extends Control {
  input: InputKind
}

export interface HudEntry {
  role: string
  name: string
}

export type HudState = 'visible' | 'hidden' | 'enabled' | 'actionable'

export interface UserData {
  name: string
  address: string
  version: number
  web3: boolean
}

export interface RenderedWearable {
  category: string
  urn: string
}

export interface EmoteState {
  urn: string
  source: 'velocitySelected' | 'triggeredEmote' | 'sceneMovementAnim'
  repeat: boolean
  finished: boolean
  mask: 'fullBody' | 'upperBody'
}

export interface AvatarReport {
  kind: 'player' | 'otherPlayer' | 'sceneAvatar' | 'preview' | 'other'
  address: string | null
  selected: { bodyShape: string | null; wearables: string[]; emotes: string[]; forceRender: string[]; fromScene: boolean }
  rendered: { processed: boolean; failed: boolean; bodyShape: string; wearables: RenderedWearable[]; hidden: string[] } | null
  emote: EmoteState | null
  upperBodyEmote: EmoteState | null
}

export interface ChatMessage {
  timestamp: number
  channel: string
  message: string
  address: string | null
  messageId: string | null
}

export interface ChatHistory {
  total: number
  messages: ChatMessage[]
}

export interface VoiceState {
  mic: { available: boolean; enabled: boolean }
  livekit: boolean
  connected: boolean | null
  rooms: { scene: boolean; state: string }[] | null
  speakers: string[] | null
}

export interface ScreenPosition {
  entity: string
  scene: string
  anchor: 'origin' | 'bounds'
  x: number | null
  y: number | null
  onScreen: boolean
  inFront: boolean
  depth: number
  distance: number
  viewport: { x: number; y: number; width: number; height: number }
}

export interface Position {
  x: number
  y: number
  z: number
}

export interface CommandRecord {
  /** The line as sent, with `/login_identity`'s payload redacted. */
  line: string
  startedAt: number
  ms: number
  reply?: string
  error?: string
}

export type CommandListener = (record: CommandRecord) => void

export interface ExplorerOptions {
  /** Accept `"input": "synthetic"` from the HUD commands (a run without the web driver). Off by
   *  default: a spec that passed on DOM events proves nothing about the browser's input path. */
  allowSyntheticInput?: boolean
}

export type MouseButton = 'left' | 'right' | 'middle'

const DEFAULT_HUD_TIMEOUT_MS = 10_000
const DEFAULT_ARRIVAL_TIMEOUT_MS = 180_000

/** The error as it may be rethrown: a transport's own error (a timeout, a closed driver) can
 *  quote the line, and the test runner prints the message and the stack. */
function scrubbed(line: string, error: unknown): unknown {
  if (!(error instanceof Error)) return error
  error.message = redact(line, error.message)
  if (error.stack !== undefined) error.stack = redact(line, error.stack)
  return error
}

export class Explorer {
  readonly console: Console
  readonly hud: Hud
  readonly #listeners = new Set<CommandListener>()
  readonly #allowSynthetic: boolean

  constructor(console: Console, options: ExplorerOptions = {}) {
    this.console = console
    this.#allowSynthetic = options.allowSyntheticInput ?? false
    this.hud = new Hud(this, this.#allowSynthetic)
  }

  /** Observe every command sent and its reply or failure (the report records them). */
  onCommand(listener: CommandListener): () => void {
    this.#listeners.add(listener)
    return () => this.#listeners.delete(listener)
  }

  /** Run a raw console line. */
  async run(line: string): Promise<string> {
    const startedAt = Date.now()
    const record: CommandRecord = { line: redactLine(line.trim()), startedAt, ms: 0 }
    try {
      const reply = await this.console.run(line)
      record.ms = Date.now() - startedAt
      record.reply = reply.length > 4000 ? `${reply.slice(0, 4000)}... (${reply.length} chars)` : reply
      return reply
    } catch (error) {
      record.ms = Date.now() - startedAt
      const safe = scrubbed(line, error)
      record.error = redact(line, safe instanceof Error ? safe.message : String(safe))
      throw safe
    } finally {
      for (const listener of this.#listeners) listener(record)
    }
  }

  /** Run a line whose reply is JSON. A reply that does not parse is an error naming the command,
   *  never silently a string. */
  async json<T>(line: string): Promise<T> {
    const reply = await this.run(line)
    try {
      return JSON.parse(reply) as T
    } catch {
      throw new Error(`\`${redactLine(line)}\` did not reply JSON: ${reply.slice(0, 200)}`)
    }
  }

  // --- world --------------------------------------------------------------------------------

  worldState(): Promise<WorldState> {
    return this.json<WorldState>('/world_state')
  }

  /** Wait until the player is in the world (signed in, realm connected, not out of world). */
  waitInWorld(timeoutMs = DEFAULT_ARRIVAL_TIMEOUT_MS): Promise<WorldState> {
    return until(() => this.worldState(), (state) => state.inWorld, { timeoutMs, what: 'the player to be in the world' })
  }

  /** Teleport to a parcel and wait for the arrival: every teleport takes the player out of the
   *  world first, so the wait is for `arrivals` to grow, then `inWorld`. */
  async teleport(x: number, y: number, options: { timeoutMs?: number } = {}): Promise<WorldState> {
    const before = (await this.worldState()).arrivals
    await this.run(`/teleport ${x} ${y}`)
    return this.#waitArrival(before, options.timeoutMs ?? DEFAULT_ARRIVAL_TIMEOUT_MS, `the arrival at ${x},${y}`)
  }

  /** Change realm (a realm URL or a world name) and wait for the arrival in it. */
  async changeRealm(realm: string, options: { timeoutMs?: number } = {}): Promise<WorldState> {
    const before = (await this.worldState()).arrivals
    await this.run(`/changerealm ${realm}`)
    return this.#waitArrival(before, options.timeoutMs ?? DEFAULT_ARRIVAL_TIMEOUT_MS, `the arrival in ${realm}`)
  }

  #waitArrival(before: number, timeoutMs: number, what: string): Promise<WorldState> {
    return until(() => this.worldState(), (state) => state.arrivals > before && state.inWorld, { timeoutMs, what })
  }

  /** Walk to a world position through the movement controller; resolves on arrival (0.5 m). */
  async walkTo(x: number, y: number, z: number, timeoutS = 20): Promise<void> {
    await this.run(`/walk_player_to ${x} ${y} ${z} ${timeoutS}`)
  }

  /** `/player_position`, parsed: DCL axes, so the parcel is `floor(x / 16), floor(z / 16)`. */
  async position(): Promise<Position> {
    const reply = await this.run('/player_position')
    const match = /\(\s*(-?[\d.]+)\s*,\s*(-?[\d.]+)\s*,\s*(-?[\d.]+)\s*\)/.exec(reply)
    if (!match) throw new Error(`unparseable player_position: ${reply}`)
    return { x: Number(match[1]), y: Number(match[2]), z: Number(match[3]) }
  }

  // --- session ------------------------------------------------------------------------------

  /** Sign in with an identity (while the world is held: the lobby, before any realm). `name` and
   *  `look` apply to a first login only. */
  async loginIdentity(identity: AuthIdentity | string, options: { name?: string; look?: number; guest?: boolean; defaultOnError?: boolean; profiles?: string } = {}): Promise<void> {
    const payload = typeof identity === 'string' ? identity : encodeIdentity(identity)
    const flags: string[] = []
    if (options.defaultOnError) flags.push('--default-on-error')
    if (options.guest) flags.push('--guest')
    if (options.name !== undefined) flags.push(`--name ${options.name}`)
    if (options.look !== undefined) flags.push(`--look ${options.look}`)
    if (options.profiles !== undefined) flags.push(`--profiles ${options.profiles}`)
    await this.run(['/login_identity', payload, ...flags].join(' '))
  }

  /** A session-only guest made in the engine (no network). Takes effect the next frame; it is
   *  ignored once signed in with the world not held, so the caller checks `/world_state`. */
  async loginGuest(): Promise<void> {
    await this.run('/login_guest')
    await until(() => this.worldState(), (state) => state.signedIn, { timeoutMs: 30_000, intervalMs: 250, what: 'the guest to be signed in' })
  }

  async logout(): Promise<void> {
    await this.run('/logout')
  }

  /** `/get_user_data`, parsed from `<name> (<address>): v<version>, web3=<bool>`. */
  async userData(address?: string): Promise<UserData> {
    const reply = await this.run(address ? `/get_user_data ${address}` : '/get_user_data')
    const match = /^(.*) \((0x[0-9a-fA-F]{40})\): v(\d+), web3=(true|false)$/s.exec(reply.trim())
    if (!match) throw new Error(`unparseable get_user_data: ${reply}`)
    return { name: match[1]!, address: match[2]!, version: Number(match[3]), web3: match[4] === 'true' }
  }

  /** The profile version, which bumps on every avatar change (an equip is observable here). */
  async profileVersion(): Promise<number> {
    return (await this.userData()).version
  }

  // --- avatar and chat ----------------------------------------------------------------------

  avatarState(which?: 'all' | string): Promise<AvatarReport[]> {
    return this.json<AvatarReport[]>(which ? `/avatar_state ${which}` : '/avatar_state')
  }

  /** The local player's avatar report. */
  async myAvatar(): Promise<AvatarReport> {
    const reports = await this.avatarState()
    const player = reports.find((report) => report.kind === 'player')
    if (!player) throw new Error('no player avatar in /avatar_state')
    return player
  }

  /** Play an emote by urn, built-in name or profile slot number. Only queues the request. */
  async emote(urnOrSlot: string | number, options: { loop?: boolean; upperBody?: boolean } = {}): Promise<void> {
    const flags = [options.loop ? '--loop' : '', options.upperBody ? '--upper-body' : ''].filter(Boolean)
    await this.run(['/emote', String(urnOrSlot), ...flags].join(' '))
  }

  chatHistory(count = 50): Promise<ChatHistory> {
    return this.json<ChatHistory>(`/chat_history ${count}`)
  }

  /** A direct message; `/dm` takes several words, `/chat` does not. Not a delivery confirmation. */
  async dm(address: string, message: string): Promise<void> {
    await this.run(`/dm ${address} ${message}`)
  }

  async dmHistory(address: string, count = 20): Promise<string[]> {
    const reply = await this.run(`/dm_history ${address} ${count}`)
    return reply === '' ? [] : reply.split('\n')
  }

  /** Other players' addresses, then the local player's. */
  async connectedPlayers(): Promise<string[]> {
    const reply = await this.run('/connected_players')
    return reply === 'no other players connected' ? [] : reply.split(',').map((address) => address.trim())
  }

  voiceState(): Promise<VoiceState> {
    return this.json<VoiceState>('/voice_state')
  }

  /** The sky time: `/time` reports; with `hours` (and `speed`, 0 stops the clock) it sets. */
  async time(hours?: number, speed?: number): Promise<{ before: string; after: string; speed: number }> {
    const reply = await this.run(['/time', hours === undefined ? '' : String(hours), speed === undefined ? '' : String(speed)].join(' '))
    const match = /^time (\d+:\d+) -> (\d+:\d+), speed ([\d.-]+)/.exec(reply)
    if (!match) throw new Error(`unparseable time: ${reply}`)
    return { before: match[1]!, after: match[2]!, speed: Number(match[3]) }
  }

  /** The engine's frame as PNG bytes (the canvas on the web, the HUD included natively). */
  async screenshot(): Promise<Buffer> {
    return Buffer.from(await this.run('/screenshot'), 'base64')
  }

  // --- input --------------------------------------------------------------------------------

  /** A key by its `KeyboardEvent.code`; `state` holds or releases it. */
  async key(code: string, state?: 'down' | 'up'): Promise<void> {
    await this.run(state ? `/press_key ${code} --state ${state}` : `/press_key ${code}`)
  }

  escape(): Promise<void> {
    return this.key('Escape')
  }

  /** A click at window pixels (as `/entity_screen_position` and `/hud_read` report them). */
  async clickAt(x: number, y: number, button: MouseButton = 'left'): Promise<void> {
    await this.run(button === 'left' ? `/click_at ${x} ${y}` : `/click_at ${x} ${y} --button ${button}`)
  }

  // --- scenes -------------------------------------------------------------------------------

  /** The target scene's live entities as `/scene_entities` lists them (`root(0)`, `512`, ...). */
  async sceneEntities(component?: string): Promise<string[]> {
    const reply = await this.run(component ? `/scene_entities ${component}` : '/scene_entities')
    return reply === '(no entities)' ? [] : reply.split('\n')
  }

  async inspect<T = unknown>(entity: string | number, component: string): Promise<T> {
    return this.json<T>(`/inspect_component ${entity} ${component}`)
  }

  entityScreenPosition(entity: string | number, options: { bounds?: boolean } = {}): Promise<ScreenPosition> {
    return this.json<ScreenPosition>(`/entity_screen_position ${entity}${options.bounds ? ' --bounds' : ''}`)
  }

  async close(): Promise<void> {
    await this.console.close()
  }
}

/** The HUD by role and accessible name, through `/hud_*`. */
export class Hud {
  readonly #explorer: Explorer
  readonly #allowSynthetic: boolean

  constructor(explorer: Explorer, allowSynthetic: boolean) {
    this.#explorer = explorer
    this.#allowSynthetic = allowSynthetic
  }

  /** The visible controls with a name, in document order, filtered by role. */
  list(role?: string): Promise<HudEntry[]> {
    return this.#explorer.json<HudEntry[]>(role ? `/hud_list ${role}` : '/hud_list')
  }

  /** A control's description; `found: false` when none (or several without an index). */
  read(selector: Selector): Promise<HudRead> {
    return this.#explorer.json<HudRead>(`/hud_read ${selectorArg(selector)}`)
  }

  /** The control, or undefined when it is not visible. Ambiguity is an error. */
  async find(selector: Selector): Promise<Control | undefined> {
    const read = await this.read(selector)
    if (read.found) return read
    if (read.error) throw new Error(`${describeSelector(selector)}: ${read.error}`)
    return undefined
  }

  async isVisible(selector: Selector): Promise<boolean> {
    return (await this.find(selector)) !== undefined
  }

  /** The control's `text`; an error when it is not visible. */
  async text(selector: Selector): Promise<string> {
    return (await this.get(selector)).text
  }

  /** The control's `value` (a field); an error when it is not visible or has none. */
  async value(selector: Selector): Promise<string> {
    const control = await this.get(selector)
    if (control.value === undefined) throw new Error(`${describeSelector(selector)} has no value: it is a ${control.role}`)
    return control.value
  }

  /** The control; an error naming it when it is not visible. */
  async get(selector: Selector): Promise<Control> {
    const control = await this.find(selector)
    if (!control) throw new Error(`no visible HUD control matches ${describeSelector(selector)}`)
    return control
  }

  /** A real click on the control's centre. Fails on a synthetic (DOM-event) click unless the
   *  explorer allows it. */
  async click(selector: Selector): Promise<HudInput> {
    const result = await this.#explorer.json<HudInput>(`/hud_click ${selectorArg(selector)}`)
    this.#checkInput(result, `click on ${describeSelector(selector)}`)
    return result
  }

  /** Replace a field's text; the reply holds the field as the typing left it. */
  async fill(selector: Selector, text: string): Promise<HudInput> {
    const result = await this.#explorer.json<HudInput>(`/hud_fill ${JSON.stringify({ ...toSelectorObject(selector), text })}`)
    this.#checkInput(result, `fill of ${describeSelector(selector)}`)
    return result
  }

  /** Wait for a control to be `visible` (default), `hidden`, `enabled` or `actionable`. */
  wait(selector: Selector, options: { state?: HudState; timeoutMs?: number } = {}): Promise<HudRead> {
    const { state = 'visible', timeoutMs = DEFAULT_HUD_TIMEOUT_MS } = options
    return this.#explorer.json<HudRead>(`/hud_wait ${JSON.stringify({ ...toSelectorObject(selector), state, timeout: timeoutMs })}`)
  }

  waitHidden(selector: Selector, timeoutMs = DEFAULT_HUD_TIMEOUT_MS): Promise<HudRead> {
    return this.wait(selector, { state: 'hidden', timeoutMs })
  }

  /** Put the pointer on a control, for the controls that appear on hover: a real click at its
   *  centre, which the browser precedes with a move. It also selects the control, as a user's
   *  click would. */
  async hover(selector: Selector): Promise<Control> {
    const control = await this.get(selector)
    await this.#explorer.clickAt(control.x + control.width / 2, control.y + control.height / 2)
    return control
  }

  #checkInput(result: HudInput, what: string): void {
    if (result.input !== 'trusted' && !this.#allowSynthetic) {
      throw new ConsoleError(what, `the ${what} was synthetic (DOM events): run under the web driver, or allow synthetic input`)
    }
  }
}
