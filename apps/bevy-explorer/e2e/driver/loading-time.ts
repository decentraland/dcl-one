// Loading time of a client measured from outside: time to world (the launch until the first ring,
// the player's scene and every scene within one parcel of the player, is ready) and teleport time
// (the teleport until the same at the destination). Readiness is the engine's own, keyed to an
// arrival (`/world_state`'s `arrivals`): `/scene_loading_state`'s `firstRing` names the latest
// arrival whose first ring has been ready and dates it (`sinceReadyMs` before the reply), so the
// poll interval adds nothing to a measurement.
import type { Explorer, SceneLoadingState } from './explorer.ts'
import { parsePosition } from './session.ts'
import { sleep, until } from './wait.ts'

export interface Location {
  name: string
  /** The parcel, `x,y`. */
  position: string
}

export interface Thresholds {
  timeToWorldMs: number
  teleportMs: number
}

export interface LoadingTimeConfig {
  thresholds: Thresholds
  locations: Location[]
}

export interface WorldMeasurement {
  location: string
  position: string
  timeToWorldMs: number | null
  error?: string
}

export interface TeleportMeasurement {
  from: string
  to: string
  teleportMs: number | null
  error?: string
}

export interface LoadingTimeResults {
  startedAt: string
  webUrl: string
  baseDomain: string
  realm: string
  thresholds: Thresholds
  worlds: WorldMeasurement[]
  teleports: TeleportMeasurement[]
}

/** How long the first ring is waited for once the player has arrived. */
export const FIRST_RING_TIMEOUT_MS = 180_000

/** The error of a client whose `/scene_loading_state` has no `firstRing`: no wait can succeed on
 *  it. */
export const FIRST_RING_UNSUPPORTED = "the client's /scene_loading_state has no firstRing"

export function parseConfig(text: string): LoadingTimeConfig {
  const config = JSON.parse(text) as Partial<LoadingTimeConfig>
  const { thresholds, locations } = config
  if (!thresholds || !(thresholds.timeToWorldMs > 0) || !(thresholds.teleportMs > 0)) {
    throw new Error('the loading-time config needs thresholds.timeToWorldMs and thresholds.teleportMs, both positive')
  }
  if (!Array.isArray(locations) || locations.length === 0) throw new Error('the loading-time config needs at least one location')
  for (const location of locations) {
    if (typeof location.name !== 'string' || location.name === '') throw new Error('every location needs a name')
    parsePosition(location.position)
  }
  return { thresholds, locations }
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

async function readSceneLoadingState(explorer: Explorer): Promise<SceneLoadingState> {
  const reply = await explorer.sceneLoadingState()
  if (!('firstRing' in reply)) throw new Error(FIRST_RING_UNSUPPORTED)
  return reply
}

/** `/world_state`'s `arrivals`, read again on a transient console error. */
async function readArrivals(explorer: Explorer, attempts = 3): Promise<number> {
  for (let attempt = 1; ; attempt++) {
    try {
      return (await explorer.worldState()).arrivals
    } catch (error) {
      if (attempt >= attempts) throw error
      await sleep(1000)
    }
  }
}

/** Wait for the first ring to be ready for arrival `arrival` or a later one, and give the
 *  `Date.now()` time it was. A client without `firstRing` fails at once; any other error is
 *  retried. */
export async function waitFirstRingReady(explorer: Explorer, options: { arrival: number; timeoutMs?: number; what: string }): Promise<number> {
  await readSceneLoadingState(explorer).catch((error: unknown) => {
    if (message(error) === FIRST_RING_UNSUPPORTED) throw error
  })
  let receivedAt = 0
  const state = await until(
    async () => {
      const reply = await explorer.sceneLoadingState()
      receivedAt = Date.now()
      return reply
    },
    (reply: SceneLoadingState) => reply.firstRing != null && reply.firstRing.arrival >= options.arrival,
    { timeoutMs: options.timeoutMs ?? FIRST_RING_TIMEOUT_MS, intervalMs: 250, what: options.what }
  )
  return receivedAt - state.firstRing!.sinceReadyMs
}

/** The first-ring wait bounded by the run's deadline. */
function firstRingTimeout(deadline: number, now: () => number): number {
  return Math.max(0, Math.min(FIRST_RING_TIMEOUT_MS, deadline - now()))
}

/** Time to world at `location`: `launch` boots a fresh client there and resolves once it is in
 *  the world, giving up when its signal aborts at `deadline`; the clock starts before it. */
export async function measureTimeToWorld(
  location: Location,
  launch: (signal: AbortSignal) => Promise<Explorer>,
  options: { deadline?: number; now?: () => number } = {}
): Promise<{ measurement: WorldMeasurement; explorer?: Explorer }> {
  const { deadline = Infinity, now = Date.now } = options
  const base = { location: location.name, position: location.position }
  const startedAt = now()
  const remaining = deadline - startedAt
  if (remaining <= 0) return { measurement: { ...base, timeToWorldMs: null, error: "skipped: the run's deadline passed" } }
  let explorer: Explorer
  try {
    explorer = await launch(Number.isFinite(remaining) ? AbortSignal.timeout(remaining) : new AbortController().signal)
  } catch (error) {
    return { measurement: { ...base, timeToWorldMs: null, error: `boot: ${message(error)}` } }
  }
  try {
    // the boot's last arrival is the one in the world at the location
    const arrival = await readArrivals(explorer)
    const readyAt = await waitFirstRingReady(explorer, { arrival, what: `the first ring at ${location.name} (${location.position})`, timeoutMs: firstRingTimeout(deadline, now) })
    return { measurement: { ...base, timeToWorldMs: readyAt - startedAt }, explorer }
  } catch (error) {
    return { measurement: { ...base, timeToWorldMs: null, error: message(error) }, explorer }
  }
}

/** Teleport time from wherever the player is to `to`. */
export async function measureTeleport(explorer: Explorer, from: Location, to: Location, options: { deadline?: number; now?: () => number } = {}): Promise<TeleportMeasurement> {
  const { deadline = Infinity, now = Date.now } = options
  const base = { from: from.name, to: to.name }
  if (deadline - now() <= 0) return { ...base, teleportMs: null, error: "skipped: the run's deadline passed" }
  try {
    const [x, y] = parsePosition(to.position)
    const sentAt = now()
    const { arrivals } = await explorer.teleport(x, y, { timeoutMs: firstRingTimeout(deadline, now) })
    const readyAt = await waitFirstRingReady(explorer, { arrival: arrivals, what: `the first ring after the teleport to ${to.name} (${to.position})`, timeoutMs: firstRingTimeout(deadline, now) })
    return { ...base, teleportMs: readyAt - sentAt }
  } catch (error) {
    return { ...base, teleportMs: null, error: message(error) }
  }
}

/** The legs of the teleport tour: from the last location to the first, then on through the list. */
export function teleportLegs(locations: Location[]): [Location, Location][] {
  if (locations.length < 2) return []
  return locations.map((to, index) => [locations[(index + locations.length - 1) % locations.length]!, to])
}

/** The readings over their threshold, one line each. */
export function slowLines(results: LoadingTimeResults): string[] {
  const lines: string[] = []
  for (const world of results.worlds) {
    if (world.timeToWorldMs !== null && world.timeToWorldMs > results.thresholds.timeToWorldMs) {
      lines.push(`time to world at ${world.location}: ${seconds(world.timeToWorldMs)} > ${seconds(results.thresholds.timeToWorldMs)}`)
    }
  }
  for (const leg of results.teleports) {
    if (leg.teleportMs !== null && leg.teleportMs > results.thresholds.teleportMs) {
      lines.push(`teleport ${leg.from} → ${leg.to}: ${seconds(leg.teleportMs)} > ${seconds(results.thresholds.teleportMs)}`)
    }
  }
  return lines
}

/** The readings that could not be taken, one line each with its error. */
export function unmeasuredLines(results: LoadingTimeResults): string[] {
  return [
    ...results.worlds.filter((w) => w.timeToWorldMs === null).map((w) => `time to world at ${w.location}: ${code(w.error ?? 'no reading', false)}`),
    ...results.teleports.filter((t) => t.teleportMs === null).map((t) => `teleport ${t.from} → ${t.to}: ${code(t.error ?? 'no reading', false)}`)
  ]
}

function seconds(ms: number | null): string {
  return ms === null ? '—' : `${(ms / 1000).toFixed(1)} s`
}

/** The run as Markdown: the readings, the thresholds, what exceeded them and what was not taken. */
export function summaryMarkdown(results: LoadingTimeResults, runUrl?: string): string {
  const slow = slowLines(results)
  const unmeasured = unmeasuredLines(results)
  const reading = (ms: number | null, error?: string): string => (error ? `failed: ${code(error, true)}` : seconds(ms))
  const out = [
    `### Loading time of the live web client, ${results.startedAt}`,
    '',
    `${results.webUrl} (realm ${results.realm})${runUrl ? `, [run](${runUrl})` : ''}`,
    '',
    `| Time to world | Reading | Threshold |`,
    `| --- | --- | --- |`,
    ...results.worlds.map((w) => `| ${w.location} (${w.position}) | ${reading(w.timeToWorldMs, w.error)} | ${seconds(results.thresholds.timeToWorldMs)} |`),
    ''
  ]
  if (results.teleports.length > 0) {
    out.push(
      `| Teleport | Reading | Threshold |`,
      `| --- | --- | --- |`,
      ...results.teleports.map((t) => `| ${t.from} → ${t.to} | ${reading(t.teleportMs, t.error)} | ${seconds(results.thresholds.teleportMs)} |`),
      ''
    )
  }
  if (slow.length > 0) out.push(`Over the threshold:\n\n${slow.map((line) => `- ${line}`).join('\n')}`, '')
  if (unmeasured.length > 0) out.push(`Not measured:\n\n${unmeasured.map((line) => `- ${line}`).join('\n')}`, '')
  if (slow.length === 0 && unmeasured.length === 0) out.push('Every reading is within its threshold.', '')
  return out.join('\n')
}

/** Error text as an inline code span on one line: no mention, reference or link renders from it.
 *  In a table cell a `|` is escaped, so it cannot end the cell. */
function code(text: string, inTable: boolean): string {
  const flat = text.replace(/\s+/g, ' ').replace(/`/g, "'").replace(/\|/g, inTable ? '\\|' : '|')
  return `\`${flat.length > 200 ? `${flat.slice(0, 200)}…` : flat}\``
}
