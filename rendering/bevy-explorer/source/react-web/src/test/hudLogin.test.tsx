// `/hud_login` (lib/hudAutomation.ts): a test runner's sign-in through the HUD's own flow. The
// session takes the identity as this page's stored sign-in and goes in as Jump In does; the
// command answers once the sign-in has landed and the welcome page is done, in the lobby or on
// the way to its destination, or with what ended it.
import { afterEach, describe, expect, it } from 'vitest'
import { act, waitFor } from '@testing-library/react'
import type { NewAccountPick } from '../engine/driver'
import type { PageToScene } from '../engine/protocol'
import type { AuthIdentity } from '../features/auth/sso'
import { PAGE_COMMANDS, loginThroughHud, parseLoginArgs, provideHudLogin } from '../lib/hudAutomation'
import { FakeDriver, renderSession } from './harness'

const ADDRESS = '0x' + 'Ab'.repeat(20)
const IDENTITY: AuthIdentity = {
  ephemeralIdentity: { address: '0x' + '12'.repeat(20), publicKey: '0x04' + '34'.repeat(64), privateKey: '0x' + '56'.repeat(32) },
  expiration: '2099-01-01T00:00:00.000Z',
  authChain: [
    { type: 'SIGNER', payload: ADDRESS, signature: '' },
    { type: 'ECDSA_EPHEMERAL', payload: `Decentraland Login\nEphemeral address: 0x${'12'.repeat(20)}\nExpiration: 2099-01-01T00:00:00.000Z`, signature: '0x' + '78'.repeat(65) }
  ]
}
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

interface Reply {
  address: string
  guest: boolean
  phase: string
}

async function login(rest: string): Promise<Reply> {
  return JSON.parse(await PAGE_COMMANDS['/hud_login'].run(rest)) as Reply
}

async function ready(driver: IdentityDriver): Promise<ReturnType<typeof renderSession>> {
  const h = renderSession({ userId: null }, driver)
  await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
  await waitFor(() => expect(h.session().login.engineReady).toBe(true))
  return h
}

afterEach(() => provideHudLogin(null))

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
    expect(message(`${ENCODED} --name tester --look x`)).toBe("--look takes a curated look's number, from 1, not `x`")
    expect(message(`${ENCODED} --name a_name --look 1`)).toBe('--name takes 1 to 15 letters and digits')
    expect(message(`${ENCODED} --position here`)).toBe('--position takes a parcel as x,y, not `here`')
    expect(message(`${ENCODED} --realm`)).toMatch(/^--realm takes a value/)
    expect(message(`${ENCODED} --jump`)).toMatch(/^unknown flag `--jump`/)
    for (const rest of ['', '--guest', `${ENCODED} --name tester`, `${ENCODED} --jump`]) expect(message(rest)).not.toContain(ENCODED)
  })

  it('fails when no session is mounted to take it', async () => {
    await expect(loginThroughHud(parseLoginArgs(ENCODED))).rejects.toThrow('no HUD session takes a sign-in here')
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

  it('stands for the terms without a pick', async () => {
    const driver = new IdentityDriver()
    driver.welcome = { terms: true, newProfile: false }
    const h = await ready(driver)
    await expect(login(ENCODED)).resolves.toMatchObject({ phase: 'lobby' })
    expect(driver.sentOf('acceptWelcome')).toEqual([{ kind: 'acceptWelcome' }])
    expect(h.session().phase).toBe('lobby')
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

  it('reports a welcome page whose accept failed', async () => {
    const driver = new IdentityDriver()
    driver.welcome = { terms: true, newProfile: true }
    driver.accept = { ok: false, error: 'deploy refused' }
    const h = await ready(driver)
    await expect(login(`${ENCODED} --name tester --look 1`)).rejects.toThrow('accepting the welcome page failed: deploy refused')
    expect(h.session().phase).toBe('welcome')
    expect(h.session().welcome.error).toBe('deploy refused')
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

  it('answers a pending sign-in that a sign-out ended', async () => {
    const driver = new IdentityDriver()
    driver.hold = true
    const h = await ready(driver)
    const first = login(ENCODED)
    await waitFor(() => expect(driver.identityLogins).toHaveLength(1))
    act(() => h.session().logout())
    await expect(first).rejects.toThrow('signed out')
    expect(h.session().login.account).toBeNull()
    driver.release()
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

  it("on a page with a link's destination the reply waits for the sign-in, and a failure is reported", async () => {
    const url = new URL(location.href)
    history.replaceState(null, '', '/?position=1,2')
    try {
      const driver = new IdentityDriver()
      driver.failWith = 'profile fetch failed: 503'
      const h = await ready(driver)
      await expect(login(ENCODED)).rejects.toThrow('profile fetch failed: 503')
      expect(driver.identityLogins).toHaveLength(1)
      await waitFor(() => expect(h.session().phase).toBe('login'))
      driver.failWith = null
      // the link's destination wins over the command's own
      const reply = await login(`${ENCODED} --realm ${REALM} --position 10,20`)
      expect(reply).toMatchObject({ address: ADDRESS.toLowerCase(), phase: 'entering' })
      expect(driver.identityLogins).toHaveLength(2)
      expect(h.session().phase).toBe('entering')
    } finally {
      history.replaceState(null, '', url.pathname + url.search)
    }
  })

  it('is refused before the engine can take a sign-in', async () => {
    const driver = new EngineNotReadyDriver()
    const h = renderSession({ userId: null }, driver)
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    await expect(login(ENCODED)).rejects.toThrow('the engine is not ready')
    expect(driver.identityLogins).toEqual([])
  })
})
