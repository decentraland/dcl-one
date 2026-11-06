// `/hud_login` (lib/hudAutomation.ts): a test runner's sign-in through the HUD's own flow. The
// session takes the identity as this page's stored sign-in and goes in as Jump In does; the
// command answers once the sign-in has landed and the welcome page is done, in the lobby or on
// the way to its destination, or with what ended it.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, waitFor } from '@testing-library/react'
import type { NewAccountPick } from '../engine/driver'
import type { PageToScene } from '../engine/protocol'
import type { AuthIdentity } from '../features/auth/sso'
import { LOGIN_COMMANDS, loginThroughHud, parseLoginArgs, provideHudLogin } from '../lib/hudAutomation'
import { FakeDriver, renderSession } from './harness'

const ADDRESS = '0x' + 'Ab'.repeat(20)
function identityOf(address: string): AuthIdentity {
  return {
    ephemeralIdentity: { address: '0x' + '12'.repeat(20), publicKey: '0x04' + '34'.repeat(64), privateKey: '0x' + '56'.repeat(32) },
    expiration: '2099-01-01T00:00:00.000Z',
    authChain: [
      { type: 'SIGNER', payload: address, signature: '' },
      { type: 'ECDSA_EPHEMERAL', payload: `Decentraland Login\nEphemeral address: 0x${'12'.repeat(20)}\nExpiration: 2099-01-01T00:00:00.000Z`, signature: '0x' + '78'.repeat(65) }
    ]
  }
}
const IDENTITY = identityOf(ADDRESS)
const ENCODED = btoa(JSON.stringify(IDENTITY))
const REALM = 'https://realm.example.test'

/** A driver whose welcome page's accept is answered, whose sign-in can be held until `release`,
 *  and can fail the way the engine driver's does: with a raw string. */
class IdentityDriver extends FakeDriver {
  failWith: string | null = null
  accept: { ok: true } | { ok: false; error: string } = { ok: true }
  hold = false
  private readonly held: (() => void)[] = []
  release(): void {
    for (const go of this.held.splice(0)) go()
  }
  async loginWithIdentity(identity: AuthIdentity, defaultOnError?: boolean, guest?: boolean, pick?: NewAccountPick): Promise<void> {
    await super.loginWithIdentity(identity, defaultOnError, guest, pick)
    if (this.hold) await new Promise<void>((go) => this.held.push(go))
    if (this.failWith != null) throw this.failWith
  }
  send(msg: PageToScene): void {
    super.send(msg)
    if (msg.kind === 'acceptWelcome') queueMicrotask(() => this.emit({ kind: 'welcomeAccepted', ...this.accept }))
  }
}

class EngineNotReadyDriver extends IdentityDriver {
  engineReady(): boolean {
    return false
  }
}

/** An engine the page launches itself (the web), as the link flow needs: without `launch` the
 *  hook takes the standalone's branch, where the engine is already at its realm. */
class LaunchingDriver extends IdentityDriver {
  launch(): void {}
}

/** An engine whose launch panics at once (the generic wasm trap), so `launchEngine` fails. */
class LaunchFailingDriver extends IdentityDriver {
  launch(): void {
    throw new Error('unreachable')
  }
}

interface Reply {
  address: string
  guest: boolean
  phase: string
}

const runnerWindow = window as Window & { __dclTrustedInput?: (request: string) => void }

async function login(rest: string): Promise<Reply> {
  return JSON.parse(await LOGIN_COMMANDS['/hud_login'].run(rest)) as Reply
}

async function ready(driver: IdentityDriver): Promise<ReturnType<typeof renderSession>> {
  const h = renderSession({ userId: null }, driver)
  await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
  await waitFor(() => expect(h.session().login.engineReady).toBe(true))
  return h
}

/** A link's page: `search` on the address until the test is over. */
function onPage(search: string): () => void {
  const url = new URL(location.href)
  history.replaceState(null, '', search)
  return () => history.replaceState(null, '', url.pathname + url.search)
}

// the test runner's hook, which the command takes as its licence
beforeEach(() => {
  runnerWindow.__dclTrustedInput = () => undefined
})
afterEach(() => {
  provideHudLogin(null)
  delete runnerWindow.__dclTrustedInput
  localStorage.clear()
})

describe('/hud_login arguments', () => {
  it('takes the identity and its flags', () => {
    const request = parseLoginArgs(`  ${ENCODED} --name tester --look 3 --guest --default-on-error --realm ${REALM} --position -10,20 `)
    expect(request).toEqual({
      identity: IDENTITY,
      guest: true,
      defaultOnError: true,
      pick: { name: 'tester', look: 3 },
      destination: { realm: REALM, position: '-10,20' }
    })
    expect(parseLoginArgs(ENCODED)).toEqual({ identity: IDENTITY, guest: false, defaultOnError: false })
  })

  it('refuses what it cannot take, without quoting the identity', () => {
    const message = (rest: string): string => {
      try {
        parseLoginArgs(rest)
      } catch (e) {
        return (e as Error).message
      }
      return 'no error'
    }
    expect(message('')).toMatch(/^expected \/hud_login <identity>/)
    expect(message('--guest')).toMatch(/^expected \/hud_login <identity>/)
    expect(message('not-base64-json')).toBe('the identity is not base64 JSON')
    expect(message(btoa('{"ephemeralIdentity":{"address":"0x1"},"authChain":[{"payload":"nope"}]}'))).toMatch(/^the identity is not an AuthIdentity/)
    expect(message(`${ENCODED} --name tester`)).toBe("a first login's pick is --name and --look together")
    expect(message(`${ENCODED} --name tester --look x`)).toBe("--look takes a curated look's number, 1 to 160, not `x`")
    expect(message(`${ENCODED} --name tester --look 161`)).toBe("--look takes a curated look's number, 1 to 160, not `161`")
    expect(message(`${ENCODED} --name a_name --look 1`)).toBe('--name takes 1 to 15 letters and digits')
    expect(message(`${ENCODED} --position here`)).toBe('--position takes a parcel as x,y, not `here`')
    expect(message(`${ENCODED} --realm`)).toMatch(/^--realm takes a value/)
    expect(message(`${ENCODED} --jump`)).toMatch(/^unknown flag `--jump`/)
    // a second word that is no flag may be another identity: never quoted
    expect(message(`${ENCODED} ${ENCODED}`)).toMatch(/^unexpected argument: \/hud_login/)
    for (const rest of ['', '--guest', `${ENCODED} --name tester`, `${ENCODED} --jump`, `${ENCODED} ${ENCODED}`]) expect(message(rest)).not.toContain(ENCODED)
  })

  it('fails when no session is mounted to take it, and without a test runner driving the browser', async () => {
    await expect(loginThroughHud(parseLoginArgs(ENCODED))).rejects.toThrow('no HUD session takes a sign-in here')
    delete runnerWindow.__dclTrustedInput
    await expect(loginThroughHud(parseLoginArgs(ENCODED))).rejects.toThrow('/hud_login takes a test runner driving the browser')
  })
})

describe('/hud_login through the session', () => {
  it('signs in as a stored login, accepts the terms for an existing account without renaming it, and goes to the destination from the lobby', async () => {
    const driver = new IdentityDriver()
    // a fresh install: the terms wait on the welcome page; the account has its profile
    driver.welcome = { terms: true, newProfile: false }
    const h = await ready(driver)

    const reply = await login(`${ENCODED} --name tester --look 2 --realm ${REALM} --position 10,20`)
    expect(reply).toEqual({ address: ADDRESS.toLowerCase(), guest: false, phase: 'entering' })
    expect(driver.identityLogins).toEqual([{ identity: IDENTITY, defaultOnError: false, guest: false, pick: { name: 'tester', look: 2 } }])
    expect(driver.calls).not.toContain('jumpIn')
    // the terms are accepted; the existing profile keeps its name (no name, no deploy)
    expect(driver.sentOf('acceptWelcome')).toEqual([{ kind: 'acceptWelcome' }])
    expect(driver.last('teleport')).toMatchObject({ realm: REALM, x: 10, y: 20 })
    expect(h.session().phase).toBe('entering')
    expect(h.session().login.error).toBeNull()
    // the identity is the page's stored sign-in now
    expect(h.session().login.account).toBe(ADDRESS)

    // signed in: a second sign-in is refused
    await expect(login(ENCODED)).rejects.toThrow('already signed in')
  })

  it("a new profile takes the pick's name; without a destination the sign-in lands in the lobby, a guest reported as one", async () => {
    const driver = new IdentityDriver()
    driver.welcome = { terms: true, newProfile: true }
    const h = await ready(driver)
    const reply = await login(`${ENCODED} --guest --name tester --look 1`)
    expect(reply).toEqual({ address: ADDRESS.toLowerCase(), guest: true, phase: 'lobby' })
    expect(driver.identityLogins[0]).toMatchObject({ guest: true, pick: { name: 'tester', look: 1 } })
    expect(driver.sentOf('acceptWelcome')).toEqual([{ kind: 'acceptWelcome', name: 'tester' }])
    expect(driver.sentOf('teleport')).toEqual([])
    expect(h.session().phase).toBe('lobby')
  })

  it('a parcel alone goes to the main realm from the lobby', async () => {
    const driver = new IdentityDriver()
    const h = await ready(driver)
    const reply = await login(`${ENCODED} --position 3,-4`)
    expect(reply).toMatchObject({ phase: 'entering' })
    const teleport = driver.last('teleport') as { realm?: string; x: number; y: number } | undefined
    expect(teleport).toMatchObject({ x: 3, y: -4 })
    expect(teleport?.realm).toBeTruthy()
    expect(h.session().phase).toBe('entering')
  })

  it('stands for the terms without a pick, for an account that has its profile', async () => {
    const driver = new IdentityDriver()
    driver.welcome = { terms: true, newProfile: false }
    const h = await ready(driver)
    await expect(login(ENCODED)).resolves.toMatchObject({ phase: 'lobby' })
    expect(driver.sentOf('acceptWelcome')).toEqual([{ kind: 'acceptWelcome' }])
    expect(h.session().phase).toBe('lobby')
  })

  it('refuses a first login without a pick, and the welcome page shows as it would to a person', async () => {
    const driver = new IdentityDriver()
    driver.welcome = { terms: true, newProfile: true }
    const h = await ready(driver)
    await expect(login(ENCODED)).rejects.toThrow('a first login needs --name and --look')
    expect(driver.sentOf('acceptWelcome')).toEqual([])
    await waitFor(() => expect(h.session().phase).toBe('welcome'))
    expect(h.session().welcome.newProfile).toBe(true)
  })

  it("reports the sign-in's failure, puts the stored sign-in back, and leaves the HUD on its start page with the error", async () => {
    const driver = new IdentityDriver()
    driver.failWith = 'profile fetch failed: 503'
    const h = await ready(driver)
    await expect(login(`${ENCODED} --realm ${REALM}`)).rejects.toThrow('profile fetch failed: 503')
    await waitFor(() => expect(h.session().phase).toBe('login'))
    expect(h.session().login.error).toBe('profile fetch failed: 503')
    expect(h.session().login.account).toBeNull()
    expect(driver.sentOf('teleport')).toEqual([])
    // the failed one is over: another can be tried
    driver.failWith = null
    await expect(login(ENCODED)).resolves.toMatchObject({ phase: 'lobby' })
    expect(h.session().login.account).toBe(ADDRESS)
  })

  it('puts a stored sign-in the page had back after a failure', async () => {
    const previous = '0x' + 'Cd'.repeat(20)
    localStorage.setItem(`single-sign-on-${previous.toLowerCase()}`, JSON.stringify(identityOf(previous)))
    const driver = new IdentityDriver()
    driver.failWith = 'profile fetch failed: 503'
    const h = await ready(driver)
    await waitFor(() => expect(h.session().login.account).toBe(previous))
    await expect(login(ENCODED)).rejects.toThrow('profile fetch failed: 503')
    // the stored sign-in goes back with the failure's state update, which lands on the next render
    await waitFor(() => expect(h.session().login.account).toBe(previous))
  })

  it('reports a welcome page whose accept failed', async () => {
    const driver = new IdentityDriver()
    driver.welcome = { terms: true, newProfile: true }
    driver.accept = { ok: false, error: 'deploy refused' }
    const h = await ready(driver)
    await expect(login(`${ENCODED} --name tester --look 1`)).rejects.toThrow('accepting the welcome page failed: deploy refused')
    expect(h.session().phase).toBe('welcome')
    expect(h.session().welcome.error).toBe('deploy refused')
  })

  it("refuses a sign-in in the same task as another's submit, before the next render", async () => {
    const driver = new IdentityDriver()
    const h = await ready(driver)
    let refused: Promise<Reply> | undefined
    act(() => {
      h.session().login.exploreAsGuest()
      refused = login(ENCODED)
    })
    await expect(refused).rejects.toThrow('already signed in')
    await waitFor(() => expect(h.session().phase).toBe('lobby'))
    expect(driver.calls).toContain('loginGuest')
    expect(driver.identityLogins).toEqual([])
  })

  it('refuses a second sign-in while one is under way', async () => {
    const driver = new IdentityDriver()
    driver.hold = true
    await ready(driver)
    const first = login(ENCODED)
    await expect(login(ENCODED)).rejects.toThrow('a /hud_login is already under way')
    await waitFor(() => expect(driver.identityLogins).toHaveLength(1))
    driver.release()
    await expect(first).resolves.toMatchObject({ phase: 'lobby' })
  })

  it('answers a pending sign-in that a sign-out ended, and says so until the engine has answered it', async () => {
    const driver = new IdentityDriver()
    driver.hold = true
    const h = await ready(driver)
    const first = login(ENCODED)
    await waitFor(() => expect(driver.identityLogins).toHaveLength(1))
    act(() => h.session().logout())
    await expect(first).rejects.toThrow('signed out')
    expect(h.session().login.account).toBeNull()
    // the engine still holds the first sign-in: a new one waits for it, and is told so
    await expect(login(ENCODED)).rejects.toThrow('a sign-in is still finishing')
    driver.hold = false
    driver.release()
    await waitFor(() => expect(h.session().login.busy).toBe(false))
    await expect(login(ENCODED)).resolves.toMatchObject({ phase: 'lobby' })
  })

  it('answers a pending sign-in the HUD went away under', async () => {
    const driver = new IdentityDriver()
    driver.hold = true
    const h = await ready(driver)
    const first = login(ENCODED)
    await waitFor(() => expect(driver.identityLogins).toHaveLength(1))
    h.unmount()
    await expect(first).rejects.toThrow('the HUD unmounted')
    driver.release()
  })

  it("on a page with a link's destination the reply waits for the sign-in and the welcome page, and a failure is reported", async () => {
    const leave = onPage('/?position=1,2')
    try {
      const driver = new IdentityDriver()
      driver.welcome = { terms: true, newProfile: false }
      driver.failWith = 'profile fetch failed: 503'
      const h = await ready(driver)
      await expect(login(ENCODED)).rejects.toThrow('profile fetch failed: 503')
      expect(driver.identityLogins).toHaveLength(1)
      await waitFor(() => expect(h.session().phase).toBe('login'))
      driver.failWith = null
      // the link's destination wins over the command's own; the terms are accepted before the reply
      const reply = await login(`${ENCODED} --realm ${REALM} --position 10,20`)
      expect(reply).toMatchObject({ address: ADDRESS.toLowerCase(), phase: 'entering' })
      expect(driver.identityLogins).toHaveLength(2)
      expect(driver.sentOf('acceptWelcome')).toEqual([{ kind: 'acceptWelcome' }])
      expect(h.session().phase).toBe('entering')
    } finally {
      leave()
    }
  })

  it("on a page with a link's destination a welcome page whose accept failed is reported", async () => {
    const leave = onPage('/?position=1,2')
    try {
      const driver = new IdentityDriver()
      driver.welcome = { terms: true, newProfile: false }
      driver.accept = { ok: false, error: 'deploy refused' }
      await ready(driver)
      await expect(login(ENCODED)).rejects.toThrow('accepting the welcome page failed: deploy refused')
    } finally {
      leave()
    }
  })

  it("on a page with a link's realm that is not reachable, the sign-in is answered with that and never runs", async () => {
    const leave = onPage('/?realm=http://127.0.0.1:9999')
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockRejectedValue(new TypeError('Failed to fetch'))
    try {
      const driver = new LaunchingDriver()
      const h = await ready(driver)
      await expect(login(ENCODED)).rejects.toThrow('The world "http://127.0.0.1:9999" isn\'t reachable right now.')
      expect(driver.identityLogins).toEqual([])
      await waitFor(() => expect(h.session().fatalError).toMatchObject({ source: 'realm' }))
      await waitFor(() => expect(h.session().login.account).toBeNull())
    } finally {
      fetchSpy.mockRestore()
      leave()
    }
  })

  it("is answered with the engine's crash, the stored sign-in put back", async () => {
    const driver = new IdentityDriver()
    driver.hold = true
    const h = await ready(driver)
    const first = login(ENCODED)
    await waitFor(() => expect(driver.identityLogins).toHaveLength(1))
    // the same-document engine: boot.js's crash watchdog calls window.__onEngineCrash directly
    act(() => {
      ;(window as Window & { __onEngineCrash?: (message: string, source: string) => void }).__onEngineCrash?.('engine stalled', 'watchdog')
    })
    await expect(first).rejects.toThrow('engine stalled')
    await waitFor(() => expect(h.session().login.account).toBeNull())
    driver.release()
  })

  it('is refused with the crash once the engine has failed', async () => {
    const driver = new IdentityDriver()
    const h = await ready(driver)
    act(() => {
      ;(window as Window & { __onEngineCrash?: (message: string, source: string) => void }).__onEngineCrash?.('engine stalled', 'watchdog')
    })
    await waitFor(() => expect(h.session().fatalError).not.toBeNull())
    await expect(login(ENCODED)).rejects.toThrow('engine stalled')
    expect(driver.identityLogins).toEqual([])
  })

  it('is answered when the engine fails to launch for it, the stored sign-in put back', async () => {
    // a link's page: no launch before the sign-in, so the launch that fails is the sign-in's own
    const leave = onPage('/?position=1,2')
    try {
      const driver = new LaunchFailingDriver()
      const h = await ready(driver)
      expect(h.session().fatalError).toBeNull()
      await expect(login(ENCODED)).rejects.toThrow('the engine failed to launch')
      expect(driver.identityLogins).toEqual([])
      await waitFor(() => expect(h.session().fatalError).not.toBeNull())
      await waitFor(() => expect(h.session().login.account).toBeNull())
    } finally {
      leave()
    }
  })

  it('is refused with the failure once the engine failed to launch for the start page', async () => {
    const driver = new LaunchFailingDriver()
    const h = await ready(driver)
    await waitFor(() => expect(h.session().fatalError).toMatchObject({ source: 'launch' }))
    await expect(login(ENCODED)).rejects.toThrow('unreachable')
    expect(driver.identityLogins).toEqual([])
  })

  it('is refused before the engine can take a sign-in', async () => {
    const driver = new EngineNotReadyDriver()
    const h = renderSession({ userId: null }, driver)
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    await expect(login(ENCODED)).rejects.toThrow('the engine is not ready')
    expect(driver.identityLogins).toEqual([])
  })
})
