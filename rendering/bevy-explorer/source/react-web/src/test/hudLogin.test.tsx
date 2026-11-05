// `/hud_login` (lib/hudAutomation.ts): a test runner's sign-in through the HUD's own flow. The
// session takes the identity as this page's stored sign-in and goes in as Jump In does; the
// command answers once the sign-in has landed, in the lobby or on the way to its destination.
import { afterEach, describe, expect, it } from 'vitest'
import { waitFor } from '@testing-library/react'
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

/** A driver whose welcome page's accept is answered, and whose sign-in can fail the way the
 *  engine driver's does: with a raw string. */
class IdentityDriver extends FakeDriver {
  failWith: string | null = null
  accept: { ok: true } | { ok: false; error: string } = { ok: true }
  async loginWithIdentity(identity: AuthIdentity, defaultOnError?: boolean, guest?: boolean, pick?: NewAccountPick): Promise<void> {
    await super.loginWithIdentity(identity, defaultOnError, guest, pick)
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

async function login(rest: string): Promise<{ address: string; guest: boolean; phase: string }> {
  return JSON.parse(await PAGE_COMMANDS['/hud_login'].run(rest)) as { address: string; guest: boolean; phase: string }
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
  it('signs in as a stored login, answers the welcome page with the pick and goes to the destination from the lobby', async () => {
    const driver = new IdentityDriver()
    // a fresh install: the terms wait on the welcome page, which the pick answers
    driver.welcome = { terms: true, newProfile: false }
    const h = await ready(driver)

    const reply = await login(`${ENCODED} --name tester --look 2 --realm ${REALM} --position 10,20`)
    expect(reply).toEqual({ address: ADDRESS.toLowerCase(), guest: false, phase: 'entering' })
    expect(driver.identityLogins).toEqual([{ identity: IDENTITY, defaultOnError: false, guest: false, pick: { name: 'tester', look: 2 } }])
    expect(driver.calls).not.toContain('jumpIn')
    expect(driver.sentOf('acceptWelcome')).toEqual([{ kind: 'acceptWelcome', name: 'tester' }])
    expect(driver.last('teleport')).toMatchObject({ realm: REALM, x: 10, y: 20 })
    expect(h.session().phase).toBe('entering')
    expect(h.session().login.error).toBeNull()

    // signed in: a second sign-in is refused
    await expect(login(ENCODED)).rejects.toThrow('already signed in')
  })

  it('without a destination it stays in the lobby, and a guest is reported as one', async () => {
    const driver = new IdentityDriver()
    driver.welcome = { terms: true, newProfile: true }
    const h = await ready(driver)
    const reply = await login(`${ENCODED} --guest`)
    expect(reply).toEqual({ address: ADDRESS.toLowerCase(), guest: true, phase: 'lobby' })
    expect(driver.identityLogins[0]).toMatchObject({ guest: true })
    // the sign-in stands for the terms; without a pick the name is left to the engine
    expect(driver.sentOf('acceptWelcome')).toEqual([{ kind: 'acceptWelcome' }])
    expect(driver.sentOf('teleport')).toEqual([])
    expect(h.session().phase).toBe('lobby')
  })

  it("reports the sign-in's failure, and the HUD is back on the start page with it", async () => {
    const driver = new IdentityDriver()
    driver.failWith = 'profile fetch failed: 503'
    const h = await ready(driver)
    await expect(login(`${ENCODED} --realm ${REALM}`)).rejects.toThrow('profile fetch failed: 503')
    await waitFor(() => expect(h.session().phase).toBe('login'))
    expect(h.session().login.error).toBe('profile fetch failed: 503')
    expect(driver.sentOf('teleport')).toEqual([])
    // the failed one is over: another can be tried
    driver.failWith = null
    await expect(login(ENCODED)).resolves.toMatchObject({ phase: 'lobby' })
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

  it('is refused before the engine can take a sign-in', async () => {
    const driver = new EngineNotReadyDriver()
    const h = renderSession({ userId: null }, driver)
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    await expect(login(ENCODED)).rejects.toThrow('the engine is not ready')
    expect(driver.identityLogins).toEqual([])
  })
})
