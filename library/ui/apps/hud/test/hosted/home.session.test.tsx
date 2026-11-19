// The home a player sets from the map is the home the session uses next: the lobby's home card and
// "Skip goes home" read the engine's answer before the snapshot the web build took at boot.
import { act, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { DEFAULT_REALM } from '@hud-host'
import { FakeDriver, enterAsGuest, renderSession } from '../harness'

describe('home in the session', () => {
  it('uses a home set from the map over the boot snapshot', async () => {
    const h = renderSession()
    Object.assign(h.driver, { homeScene: () => ({ realm: 'boot-home.dcl.eth', parcel: '1,1' }) })
    await enterAsGuest(h)
    expect(h.session().homeScene()).toEqual({ realm: 'boot-home.dcl.eth', parcel: '1,1' })

    act(() => h.session().map.setHome(DEFAULT_REALM, { x: -20, y: 60 }))
    expect(h.driver.last('setHomeScene')).toEqual({ kind: 'setHomeScene', realm: DEFAULT_REALM, parcel: { x: -20, y: 60 } })
    expect(h.session().map.home).toEqual({ realm: null, parcel: '-20,60' })
    expect(h.session().homeScene()).toEqual({ realm: null, parcel: '-20,60' })

    // the engine's answer stands, whatever the page showed
    act(() => h.driver.emit({ kind: 'homeScene', realm: 'mansion.dcl.eth', parcel: { x: 0, y: 0 } }))
    expect(h.session().map.home).toEqual({ realm: 'mansion.dcl.eth', parcel: '0,0' })
  })

  it('reads a boot snapshot saved on the default realm as Genesis City, on the map too, before the engine answers', async () => {
    const driver = new FakeDriver()
    Object.assign(driver, { homeScene: () => ({ realm: DEFAULT_REALM, parcel: '3,4' }) })
    const h = renderSession(undefined, driver)
    await waitFor(() => expect(h.session().map.home).toEqual({ realm: null, parcel: '3,4' }))
    expect(h.session().homeScene()).toEqual({ realm: null, parcel: '3,4' })
    expect(driver.last('getHomeScene')).toBeUndefined()
  })
})
