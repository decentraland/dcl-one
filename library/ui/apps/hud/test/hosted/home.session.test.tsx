// The home a player sets from the map is the home the session uses next: the lobby's home card and
// "Skip goes home" read the engine's answer before the snapshot the web build took at boot.
import { act, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { DEFAULT_REALM } from '@hud-host'
import { FakeDriver, enterAsGuest, renderSession } from '../harness'

describe('home in the session', () => {
  it('uses a home set from the map over the boot snapshot', async () => {
    const h = renderSession()
    Object.assign(h.driver, { homeScene: () => ({ realm: 'boot-home.dcl.eth', parcel: '1,1', pinned: true }) })
    await enterAsGuest(h)
    expect(h.session().homeScene()).toEqual({ realm: 'boot-home.dcl.eth', parcel: '1,1', pinned: true })

    act(() => h.session().map.setHome(DEFAULT_REALM, { x: -20, y: 60 }))
    expect(h.driver.last('setHomeScene')).toEqual({ kind: 'setHomeScene', realm: DEFAULT_REALM, parcel: { x: -20, y: 60 } })
    expect(h.session().map.home).toEqual({ realm: null, parcel: '-20,60', pinned: true })
    expect(h.session().homeScene()).toEqual({ realm: null, parcel: '-20,60', pinned: true })

    // the engine's answer stands, whatever the page showed
    act(() => h.driver.emit({ kind: 'homeScene', realm: 'mansion.dcl.eth', parcel: { x: 0, y: 0 }, pinned: true }))
    expect(h.session().map.home).toEqual({ realm: 'mansion.dcl.eth', parcel: '0,0', pinned: true })
  })

  it('takes the home back, and holds no home once the engine says the account has none', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().map.setHome(DEFAULT_REALM, { x: -20, y: 60 }))
    act(() => h.session().map.clearHome())
    expect(h.driver.last('clearHomeScene')).toEqual({ kind: 'clearHomeScene' })
    expect(h.session().map.home).toEqual({ realm: null, parcel: '0,0', pinned: false })

    act(() => h.driver.emit({ kind: 'homeScene', realm: DEFAULT_REALM, parcel: { x: 0, y: 0 }, pinned: false }))
    expect(h.session().map.home).toEqual({ realm: null, parcel: '0,0', pinned: false })
  })

  it("asks for the home again once the sign-in lands, so Skip goes to the signed-in account's", async () => {
    const driver = new FakeDriver()
    let signedIn = (): void => {}
    driver.loginGuest = () => {
      driver.calls.push('loginGuest')
      return new Promise<void>((resolve) => (signedIn = resolve))
    }
    const h = renderSession(undefined, driver)
    await waitFor(() => expect(h.session().login.status).not.toBe('loading'))
    act(() => h.session().login.exploreAsGuest())
    await waitFor(() => expect(driver.calls).toContain('loginGuest'))
    expect(driver.sentOf('getHomeScene')).toHaveLength(1)
    act(() => driver.emit({ kind: 'homeScene', realm: 'theirs.dcl.eth', parcel: { x: 5, y: 6 }, pinned: true }))
    expect(h.session().homeScene()).toEqual({ realm: 'theirs.dcl.eth', parcel: '5,6', pinned: true })

    await act(async () => signedIn())
    await waitFor(() => expect(h.session().login.busy).toBe(false))
    expect(driver.sentOf('getHomeScene')).toHaveLength(2)
    act(() => driver.emit({ kind: 'homeScene', realm: DEFAULT_REALM, parcel: { x: 0, y: 0 }, pinned: false }))
    expect(h.session().homeScene()).toEqual({ realm: null, parcel: '0,0', pinned: false })

    act(() => h.session().pickDestination(null))
    expect(driver.last('teleport')).toMatchObject({ kind: 'teleport', realm: DEFAULT_REALM, x: 0, y: 0 })
  })

  it('reads a boot snapshot saved on the default realm as Genesis City, on the map too, before the engine answers', async () => {
    const driver = new FakeDriver()
    Object.assign(driver, { homeScene: () => ({ realm: DEFAULT_REALM, parcel: '3,4', pinned: true }) })
    const h = renderSession(undefined, driver)
    await waitFor(() => expect(h.session().map.home).toEqual({ realm: null, parcel: '3,4', pinned: true }))
    expect(h.session().homeScene()).toEqual({ realm: null, parcel: '3,4', pinned: true })
    expect(driver.last('getHomeScene')).toBeUndefined()
  })
})
