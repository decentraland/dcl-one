// One client per process and realm: the first scenario boots it (serves the web app, starts
// Chrome through the web driver or attaches to the standalone's loopback console), signs in,
// waits for the world and reaches the realm and the parcel; every later scenario shares it, as
// explorer-automation keeps one client for a whole run. `--test-isolation=none` keeps the spec
// files in one process.
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { chromeArgs } from './chrome.ts'
import { type Console, ConsoleError, scrubCredentials } from './console.ts'
import { Explorer, type HudLoginReply, type WorldState } from './explorer.ts'
import { buildAuthIdentity, newPrivateKey } from './identity.ts'
import { NativeConsole, spawnNative } from './native.ts'
import { serveWebApp, type StaticServer } from './serve.ts'
import { accountKey, describeTarget, readTarget, type Target } from './target.ts'
import { until } from './wait.ts'
import { BootError, WebConsole } from './web.ts'
import { REPORT_DIR } from './paths.ts'

export type SignIn = 'account' | 'guest' | 'none'

export interface OpenOptions {
  /** The realm to boot into; the target's by default. */
  realm?: string
  position?: string
  windowSize?: [number, number]
  signIn?: SignIn
  /** The first login's profile name (1 to 15 alphanumerics) and curated look. */
  firstLogin?: { name?: string; look?: number }
}

export interface Session {
  readonly explorer: Explorer
  readonly target: Target
  readonly realm: string
  readonly url?: string
  close(): Promise<void>
}

const sessions = new Map<string, Promise<Session>>()
/** The boots of a session that failed, by its key. */
const bootFailures = new Map<string, { count: number; error: unknown }>()
/** How many times a client is booted for one session: past it, its scenarios fail at once. */
export const MAX_BOOTS = 2
let webServer: Promise<StaticServer> | undefined

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function key(target: Target, options: OpenOptions): string {
  const size = options.windowSize ?? target.windowSize
  return [options.realm ?? target.realm, options.position ?? target.position, size.join('x'), options.signIn ?? 'account'].join('|')
}

/** The client for these options, booted on first use. The harness closes every session once the
 *  whole run is done (its module-level `after`): a hook registered from inside a scenario would
 *  belong to that scenario and close the client after it. */
export function openSession(options: OpenOptions = {}, target: Target = readTarget()): Promise<Session> {
  const id = key(target, options)
  let session = sessions.get(id)
  if (!session) {
    const failed = bootFailures.get(id)
    if (failed && failed.count >= MAX_BOOTS) {
      return Promise.reject(new Error(`the client failed to boot ${failed.count} times; not booting it again in this run: ${message(failed.error)}`))
    }
    session = boot(options, target).catch(async (error: unknown) => {
      sessions.delete(id)
      const count = (bootFailures.get(id)?.count ?? 0) + 1
      bootFailures.set(id, { count, error })
      // Said at once: the run's own failure summary comes at its end, which a step timeout cuts off.
      console.error(`[e2e] boot ${count} of ${MAX_BOOTS} failed: ${message(error)}`)
      if (error instanceof BootError) await keepBootFailure(count, error).catch((problem: unknown) => console.error(`[e2e] could not keep the boot's evidence: ${message(problem)}`))
      throw error
    })
    sessions.set(id, session)
  }
  return session
}

/** The Explorer of the default session. */
export async function openExplorer(options: OpenOptions = {}): Promise<Explorer> {
  return (await openSession(options)).explorer
}

export async function closeSessions(): Promise<void> {
  const open = [...sessions.values()]
  sessions.clear()
  bootFailures.clear()
  for (const session of open) {
    try {
      await (await session).close()
    } catch {
      // a client that is already gone
    }
  }
  if (webServer) {
    const server = webServer
    webServer = undefined
    try {
      await (await server).close()
    } catch {
      // a server that never came up
    }
  }
}

/** A failed boot's evidence in the report: what the page said (`boot-<n>.console.txt`) and how it
 *  looked (`boot-<n>.jpg`), under `report/boot-failures/`; the console's last lines go to the log
 *  as well, where a step timeout can still cut the run off before the report is uploaded. */
export async function keepBootFailure(boot: number, error: BootError, dir = path.join(REPORT_DIR, 'boot-failures')): Promise<void> {
  await mkdir(dir, { recursive: true })
  // the report and the log are public: a line can carry a credential (a room's access token)
  const lines = error.console.map(scrubCredentials)
  await writeFile(path.join(dir, `boot-${boot}.console.txt`), `${error.message}\n\n${lines.join('\n')}\n`)
  if (error.screenshot) await writeFile(path.join(dir, `boot-${boot}.jpg`), error.screenshot)
  const tail = lines.slice(-20)
  console.error(`[e2e] the page's console, the last ${tail.length} of ${lines.length} lines (all of them in ${dir}):\n${tail.join('\n')}`)
}

/** How long the world is waited for once signed in. */
export const WORLD_TIMEOUT_MS = 300_000

/** The HUD's world, by the rail's top button: the HUD puts it up once its own phase reaches the
 *  world, which can come after the engine's. */
export const WORLD_HUD = { name: 'Sidebar settings', role: 'button' } as const

/** How long the HUD's world is waited for once the engine is in the world. */
export const HUD_TIMEOUT_MS = 60_000

/** How long a boot may take, the sum of its waits: the web transport's `ready` (300 s), the
 *  sign-in command (120 s), the world (`WORLD_TIMEOUT_MS`) and the HUD's (`HUD_TIMEOUT_MS`). A
 *  scenario's own budget starts after it. */
export const BOOT_ALLOWANCE_MS = 780_000

async function boot(options: OpenOptions, target: Target): Promise<Session> {
  const realm = options.realm ?? target.realm
  const position = options.position ?? target.position
  console.error(`[e2e] booting ${describeTarget(target)} realm=${realm} position=${position}`)
  let console_: Console
  let url: string | undefined
  if (target.transport === 'native') {
    console_ = await nativeConsole(target, realm, position)
  } else {
    const base = target.webUrl ?? (await serveApp(target)).url
    const page = new URL(base)
    // No `realm` or `position` on the url: a destination there makes the HUD launch the engine
    // only once a sign-in is submitted on its start page (useEngineSession's url destination),
    // and the console the sign-in below needs comes up only with the engine (engine.js installs
    // it once the engine worker runs). Without one the engine launches at once for a visitor
    // with no session, holding the world; `/hud_login` then signs in through the HUD and goes to
    // the realm and the parcel from the lobby, as a link's destination would.
    page.searchParams.set('baseDomain', target.baseDomain)
    url = page.toString()
    console_ = await WebConsole.launch({
      url,
      chrome: target.chrome,
      headless: !target.headed,
      args: chromeArgs(),
      windowSize: options.windowSize ?? target.windowSize
    })
  }
  const explorer = new Explorer(console_)
  try {
    await signIn(explorer, target, options.signIn ?? 'account', options.firstLogin, { realm, position })
    if (target.transport === 'web') {
      // the HUD's travel lands there; a realm that ignores the parcel (a world) is corrected by console
      await arrive(explorer, realm, position)
      // the engine is in the world before the HUD has put up its own (the rail, and the chat,
      // which opens with its input focused): a key sent sooner can land in that input
      await explorer.hud.wait(WORLD_HUD, { timeoutMs: HUD_TIMEOUT_MS })
    }
  } catch (error) {
    // a page that opened but never arrived: what it said and how it looks, with the error; the
    // console with the credentials the sign-in sent replaced, should the page have quoted one
    const evidence = console_ instanceof WebConsole ? await console_.evidence().catch(() => undefined) : undefined
    await explorer.close()
    if (evidence) throw new BootError(message(error), evidence.console.map((line) => explorer.redactSecrets(line)), evidence.screenshot)
    throw error
  }
  return {
    explorer,
    target,
    realm,
    url,
    close: () => explorer.close()
  }
}

function serveApp(target: Target): Promise<StaticServer> {
  // A server that fails to come up (the port in use) is not kept: the next boot tries again, and
  // `closeSessions` has nothing to close.
  webServer ??= serveWebApp(target.webApp, target.ports.web).catch((error: unknown) => {
    webServer = undefined
    throw error
  })
  return webServer
}

async function nativeConsole(target: Target, realm: string, position: string): Promise<Console> {
  const { bin, port, token, args } = target.native
  if (token && !bin) return new NativeConsole({ port, token })
  if (!bin) throw new Error('set DCL_E2E_NATIVE_BIN (an automation build to start) or DCL_E2E_NATIVE_PORT and DCL_E2E_NATIVE_TOKEN (a running one)')
  return spawnNative({ bin, port, token, args: ['--realm', realm, '--position', position, ...args] })
}

/** Sign in and wait for the world. On the web the sign-in goes through the HUD (`/hud_login`):
 *  the engine's own `/login_identity` leaves the HUD on its start page with the world held, as
 *  the HUD derives its phase from its own flow. The HUD then goes to `destination` from the lobby,
 *  which releases the hold; a guest is a fresh account the engine marks as one, as the HUD's own
 *  guest is. The standalone signs in by console at its `--realm` and `--position`, and waits for
 *  the world as `enterWorld` does. */
export async function signIn(
  explorer: Explorer,
  target: Target,
  mode: SignIn,
  firstLogin: { name?: string; look?: number } = {},
  destination: { realm: string; position: string } = { realm: target.realm, position: target.position }
): Promise<WorldState> {
  const pick = { name: firstLogin.name ?? 'e2etester', look: firstLogin.look ?? 1 }
  if (target.transport === 'web' && mode !== 'none') {
    const identity = buildAuthIdentity(mode === 'account' ? accountKey(target) : newPrivateKey())
    let reply: HudLoginReply
    try {
      reply = await explorer.hudLogin(identity, { ...pick, guest: mode === 'guest', ...destination })
    } catch (error) {
      // a web app whose HUD predates the command: the page has none to run, so the engine
      // answers. No falling back to /login_identity, which leaves the HUD on its start page.
      if (error instanceof ConsoleError && error.output.includes('Command not recognized')) {
        throw new Error(
          'the web app under test has no /hud_login: its HUD predates the sign-in through it, or no test runner hook (window.__dclTrustedInput) was there when the page installed its commands; serve a build that has it (DCL_E2E_WEB_APP or DCL_E2E_WEB_URL) under the web driver',
          { cause: error }
        )
      }
      throw error
    }
    // a destination the HUD did not take (its lobby's pick did not go out) is known now, not
    // after the world's wait
    if (reply.phase !== 'entering' && reply.phase !== 'world') {
      throw new Error(`the HUD did not take the destination ${destination.realm} at ${destination.position}: it answered the sign-in in phase ${reply.phase}`)
    }
    // a first login deploys its new profile: the catalyst's window starts here
    explorer.profileDeployed()
    return explorer.waitInWorld(WORLD_TIMEOUT_MS)
  }
  if (mode === 'account') {
    await explorer.loginIdentity(buildAuthIdentity(accountKey(target)), pick)
  } else if (mode === 'guest') {
    await explorer.loginGuest()
  }
  return enterWorld(explorer, WORLD_TIMEOUT_MS)
}

/** `x,y` as a parcel. */
export function parsePosition(position: string): [number, number] {
  const match = /^\s*(-?\d+)\s*,\s*(-?\d+)\s*$/.exec(position)
  if (!match) throw new Error(`a position is "x,y", not "${position}"`)
  return [Number(match[1]), Number(match[2])]
}

/** Whether two realm names mean the same realm: a url's trailing slash and case aside. */
export function sameRealm(a: string, b: string): boolean {
  const norm = (realm: string) => realm.trim().toLowerCase().replace(/\/+$/, '')
  return norm(a) === norm(b)
}

/** Reach the realm and the parcel through the console, after the lobby let the player in
 *  wherever it did: a realm change when the player is in another, then the teleport, unless the
 *  player already stands in the parcel (a teleport there brings no arrival to wait for). */
export async function arrive(explorer: Explorer, realm: string, position: string): Promise<WorldState> {
  const [x, y] = parsePosition(position)
  let state = await explorer.worldState()
  if (!sameRealm(state.realm, realm)) state = await explorer.changeRealm(realm)
  const here = state.position && [Math.floor(state.position[0] / 16), Math.floor(state.position[2] / 16)]
  if (here && here[0] === x && here[1] === y) return state
  return explorer.teleport(x, y)
}

/** Wait for the world, pressing the lobby's `Jump In` when it offers one (the standalone's). */
export async function enterWorld(explorer: Explorer, timeoutMs = WORLD_TIMEOUT_MS): Promise<WorldState> {
  let jumped = false
  return until(
    async () => {
      const state = await explorer.worldState()
      if (state.inWorld || jumped) return state
      const jump = await explorer.hud.find({ name: 'Jump In', role: 'button', index: 0 }).catch(() => undefined)
      if (jump?.actionable) {
        await explorer.hud.click({ name: 'Jump In', role: 'button', index: 0 })
        jumped = true
      }
      return state
    },
    (state) => state.inWorld,
    { timeoutMs, intervalMs: 1000, what: 'the world after sign-in' }
  )
}
