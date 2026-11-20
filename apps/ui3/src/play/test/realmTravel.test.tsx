import { describe, it, expect } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import { SceneLoadingOverlay } from '@hud/features/session/SceneLoadingOverlay'
import { DEFAULT_REALM } from '../lib/baseDomain'
import { enterAsGuest, renderSession } from '@hud/test/harness'
import { GENESIS_SATELLITE_VIEW } from '@explorer-core/features/map/atlas'

// Audit P0 onboarding-realm-1: a failed in-world realm change stranded the player on "Reconnecting…".
// The engine now validates the destination before leaving the current realm and reports the outcome;
// the HUD shows its loader from the request until that outcome arrives.
describe('in-world realm change', () => {
  it('sends the change at once and shows the loader until the engine answers', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    expect(h.session().phase).toBe('world')

    act(() => h.session().map.changeRealm('boedo.dcl.eth'))
    expect(h.driver.sentOf('changeRealm')).toEqual([{ kind: 'changeRealm', realm: 'boedo.dcl.eth', travelId: 2 }])
    expect(h.session().travellingTo).toBe('boedo.dcl.eth')
    expect(h.session().phase).toBe('entering')

    h.driver.emit({ kind: 'travelResult', travelId: 2, realm: 'boedo.dcl.eth', ok: true })
    expect(h.session().travellingTo).toBeNull()
    expect(h.session().travelError).toBeNull()
  })

  it('drops the loader and explains a failure; the player never left', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().map.changeRealm('noexiste.dcl.eth'))
    h.driver.emit({ kind: 'travelResult', travelId: 2, realm: 'noexiste.dcl.eth', ok: false, message: 'status: 404 Not Found' })
    expect(h.session().travellingTo).toBeNull()
    expect(h.session().travelError).toBe('Couldn\'t travel to "noexiste.dcl.eth": status: 404 Not Found')
    await waitFor(() => expect(h.session().phase).toBe('world'))
    act(() => h.session().dismissTravelError())
    expect(h.session().travelError).toBeNull()
  })

  it('only the latest travel counts: a superseded one neither drops the loader nor shows an error', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().map.changeRealm('first.dcl.eth'))
    act(() => h.session().map.changeRealm('second.dcl.eth'))
    h.driver.emit({ kind: 'travelResult', travelId: 2, realm: 'first.dcl.eth', ok: false, message: 'superseded by a later realm change' })
    expect(h.session().travellingTo).toBe('second.dcl.eth')
    expect(h.session().travelError).toBeNull()
  })

  it('/world and /goto genesis in chat travel the same way', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().chat.send('/world boedo.dcl.eth'))
    act(() => h.session().chat.send('/goto genesis'))
    expect(h.driver.sentOf('changeRealm')).toEqual([
      { kind: 'changeRealm', realm: 'boedo.dcl.eth', travelId: 2 },
      { kind: 'changeRealm', realm: DEFAULT_REALM, travelId: 3 }
    ])
    expect(h.session().travellingTo).toBe(DEFAULT_REALM)
  })

  // the system lines after the greeting the chat opens with
  const systemLines = (h: ReturnType<typeof renderSession>): string[] =>
    h
      .session()
      .chat.nearby.filter((line) => line.sender === '')
      .map((line) => line.message)
      .slice(1)

  it('/goto answers in chat once the engine does', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().chat.send('/goto mana/3,4/door'))
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', realm: 'mana.dcl.eth', x: 3, y: 4, spawnPoint: 'door', travelId: 2 }])
    expect(systemLines(h)).toEqual([])
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 2, realm: 'mana.dcl.eth', ok: true }))
    expect(systemLines(h)).toEqual(['🟢 Welcome to the mana world!'])

    act(() => h.session().chat.send('/goto nowhere'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 3, realm: 'nowhere.dcl.eth', ok: false, message: 'status: 404 Not Found' }))
    expect(systemLines(h).at(-1)).toBe("🔴 Error: The world nowhere doesn't exist or not reachable!")
  })

  it('/goto x,y is a Genesis City parcel: a realm change from a world, a teleport in the city', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    // the realm of the test session advertises no Genesis City map: a world
    act(() => h.session().chat.send('/goto 10,20'))
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', realm: DEFAULT_REALM, x: 10, y: 20, travelId: 2 }])
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 2, realm: DEFAULT_REALM, ok: true }))
    expect(systemLines(h)).toEqual(['🟢 You teleported to 10,20.'])

    act(() => h.driver.emit({ kind: 'realmInfo', realm: 'main', satelliteView: GENESIS_SATELLITE_VIEW }))
    act(() => h.session().chat.send('/goto 10,20/plaza'))
    expect(h.driver.sentOf('teleport')[1]).toEqual({ kind: 'teleport', x: 10, y: 20, spawnPoint: 'plaza', travelId: -1 })
    // no realm is left: no loader
    expect(h.session().travellingTo).toBeNull()
    act(() => h.driver.emit({ kind: 'travelResult', travelId: -1, realm: '', ok: false, message: 'Something went wrong' }))
    expect(systemLines(h).at(-1)).toBe('🔴 Error: Something went wrong')

    act(() => h.session().chat.send('/goto 152,0'))
    expect(h.driver.sentOf('teleport')).toHaveLength(2)
    expect(systemLines(h).at(-1)).toBe('🔴 Error: Parcel (152, 0) is outside of the bounds.')
  })

  it('/goto-local stays in the realm the player is in', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().chat.send('/goto-local 1,2'))
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', x: 1, y: 2, travelId: -1 }])
    expect(h.driver.sentOf('changeRealm')).toEqual([])
    act(() => h.driver.emit({ kind: 'travelResult', travelId: -1, realm: '', ok: true }))
    expect(systemLines(h)).toEqual(['🟢 You teleported to 1,2.'])
    act(() => h.session().chat.send('/goto-local random'))
    expect(systemLines(h).at(-1)).toContain('🔴 Invalid parameters, usage:')
  })

  it('the loader names the destination while travelling', () => {
    render(<SceneLoadingOverlay scene={{ visible: false, realmConnected: true, title: 'Genesis Plaza', pendingAssets: null }} progress={5} travellingTo="boedo.dcl.eth" />)
    expect(screen.getByText(/travelling to boedo\.dcl\.eth/i)).toBeInTheDocument()
    expect(screen.queryByText('Genesis Plaza')).toBeNull()
  })
})
