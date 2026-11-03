// One client per process and realm: the first scenario boots it (serves the web app, starts
// Chrome through the web driver or attaches to the standalone's loopback console), signs in and
// waits for the world; every later scenario shares it, as explorer-automation keeps one client
// for a whole run. `--test-isolation=none` keeps the spec files in one process.
import { chromeArgs } from './chrome.ts'
import type { Console } from './console.ts'
import { Explorer, type WorldState } from './explorer.ts'
import { buildAuthIdentity } from './identity.ts'
import { NativeConsole, spawnNative } from './native.ts'
import { serveWebApp, type StaticServer } from './serve.ts'
import { accountKey, describeTarget, readTarget, type Target } from './target.ts'
import { until } from './wait.ts'
import { WebConsole } from './web.ts'

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
let webServer: Promise<StaticServer> | undefined

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
    session = boot(options, target).catch((error) => {
      sessions.delete(id)
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

/** How long a boot may take, the sum of its waits: the web transport's `ready` (300 s), the
 *  sign-in command (120 s) and `enterWorld` (300 s). A scenario's own budget starts after it. */
export const BOOT_ALLOWANCE_MS = 720_000

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
    page.searchParams.set('realm', realm)
    page.searchParams.set('position', position)
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
    await signIn(explorer, target, options.signIn ?? 'account', options.firstLogin)
  } catch (error) {
    await explorer.close()
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

/** Sign in and enter the world. With the world held by the lobby, the page's `Jump In` lands. */
export async function signIn(explorer: Explorer, target: Target, mode: SignIn, firstLogin: { name?: string; look?: number } = {}): Promise<WorldState> {
  if (mode === 'account') {
    await explorer.loginIdentity(buildAuthIdentity(accountKey(target)), { name: firstLogin.name ?? 'e2etester', look: firstLogin.look ?? 1 })
  } else if (mode === 'guest') {
    await explorer.loginGuest()
  }
  return enterWorld(explorer)
}

/** Wait for the world, pressing the lobby's `Jump In` when it offers one. */
export async function enterWorld(explorer: Explorer, timeoutMs = 300_000): Promise<WorldState> {
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
