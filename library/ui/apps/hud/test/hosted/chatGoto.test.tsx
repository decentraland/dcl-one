// DOMAIN: chat's /goto and /goto-local in the session — which request each destination becomes in
// Genesis City, in a world and in any other realm, the realm it is read against during and right
// after a travel, and the one line each answers with when the engine does.
import { describe, expect, it, vi } from 'vitest'
import { act, waitFor } from '@testing-library/react'
import { DEFAULT_REALM } from '@hud-host'
import { PLATFORM_BANNED_REFUSAL } from '@explorer-core/engine/protocol'
import { GENESIS_SATELLITE_VIEW } from '@explorer-core/features/map/atlas'
import { enterAsGuest, renderSession } from '../harness'

type Harness = ReturnType<typeof renderSession>

// the system lines after the greeting the chat opens with
const systemLines = (h: Harness): string[] =>
  h
    .session()
    .chat.nearby.filter((line) => line.sender === '')
    .map((line) => line.message)
    .slice(1)

const inWorld = (h: Harness, world: string): void => act(() => h.driver.emit({ kind: 'realmInfo', realm: world, satelliteView: null }))
const inGenesis = (h: Harness): void => act(() => h.driver.emit({ kind: 'realmInfo', realm: 'main', satelliteView: GENESIS_SATELLITE_VIEW }))
// a preview's realm: no Genesis City map, no world's name
const inPreview = (h: Harness): void => act(() => h.driver.emit({ kind: 'realmInfo', realm: 'http://localhost:8000', satelliteView: null }))

const CANCELED = '🔴 Error: The operation was canceled!'

describe("chat's /goto", () => {
  it('answers in chat once the engine does, and a failure is told there alone', async () => {
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
    // the chat line is the one report: no travel notice beside it
    expect(h.session().travelError).toBeNull()
    expect(h.session().travellingTo).toBeNull()

    // any other refusal in the engine's words
    act(() => h.session().chat.send('/goto locked'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 4, realm: 'locked.dcl.eth', ok: false, message: 'You are not on the access list' }))
    expect(systemLines(h).at(-1)).toBe('🔴 Error: You are not on the access list')
    expect(h.session().travelError).toBeNull()
  })

  it('names a world only when the destination is one: Genesis City and an address answer as themselves', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inWorld(h, 'boedo.dcl.eth')
    act(() => h.session().chat.send('/goto genesis'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 2, realm: DEFAULT_REALM, ok: false, message: 'status: 404 Not Found' }))
    expect(systemLines(h)).toEqual(['🔴 Error: status: 404 Not Found'])
    act(() => h.session().chat.send('/goto genesis'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 3, realm: DEFAULT_REALM, ok: true }))
    expect(systemLines(h).at(-1)).toBe('🟢 Welcome to Genesis City!')

    act(() => h.session().chat.send('/goto https://realm.example/other'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 4, realm: 'https://realm.example/other', ok: false, message: 'status: 404 Not Found' }))
    expect(systemLines(h).at(-1)).toBe('🔴 Error: status: 404 Not Found')
    act(() => h.session().chat.send('/goto https://realm.example/other'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 5, realm: 'https://realm.example/other', ok: true }))
    expect(systemLines(h).at(-1)).toBe('🟢 Welcome to https://realm.example/other!')
  })

  it('main is Genesis City, as genesis is', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inWorld(h, 'boedo.dcl.eth')
    act(() => h.session().chat.send('/goto main/100,100'))
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', realm: DEFAULT_REALM, x: 100, y: 100, spawnPoint: undefined, travelId: 2 }])
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 2, realm: DEFAULT_REALM, ok: true }))
    expect(systemLines(h)).toEqual(['🟢 Welcome to Genesis City!'])
  })

  it('x,y is a parcel of the realm the player is in: a teleport inside a world, and in the city', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inWorld(h, 'boedo.dcl.eth')
    act(() => h.session().chat.send('/goto 10,20'))
    // the world is not left: no realm, no loader
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', x: 10, y: 20, spawnPoint: undefined, travelId: -1 }])
    expect(h.driver.sentOf('changeRealm')).toEqual([])
    expect(h.session().travellingTo).toBeNull()
    act(() => h.driver.emit({ kind: 'travelResult', travelId: -1, realm: '', ok: true }))
    expect(systemLines(h)).toEqual(['🟢 You teleported to 10,20.'])
    // held to the world's bounds, not Genesis City's
    act(() => h.session().chat.send('/goto 160,155'))
    expect(h.driver.sentOf('teleport')).toHaveLength(1)
    expect(systemLines(h).at(-1)).toBe('🔴 Error: Parcel (160, 155) is outside of the bounds.')

    inGenesis(h)
    act(() => h.session().chat.send('/goto 10,20/plaza'))
    expect(h.driver.sentOf('teleport')[1]).toEqual({ kind: 'teleport', x: 10, y: 20, spawnPoint: 'plaza', travelId: -2 })
    // no realm is left: no loader
    expect(h.session().travellingTo).toBeNull()
    act(() => h.driver.emit({ kind: 'travelResult', travelId: -2, realm: '', ok: false, message: 'Something went wrong' }))
    expect(systemLines(h).at(-1)).toBe('🔴 Error: Something went wrong')

    act(() => h.session().chat.send('/goto 152,0'))
    expect(h.driver.sentOf('teleport')).toHaveLength(2)
    expect(systemLines(h).at(-1)).toBe('🔴 Error: Parcel (152, 0) is outside of the bounds.')
  })

  it('x,y in a scene preview, or before the realm is known, moves inside the realm', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    // the realm entered, before the engine reports on it
    act(() => h.session().chat.send('/goto 2,3'))
    inPreview(h)
    act(() => h.session().chat.send('/goto 4,5'))
    expect(h.driver.sentOf('teleport')).toEqual([
      { kind: 'teleport', x: 2, y: 3, travelId: -1 },
      { kind: 'teleport', x: 4, y: 5, travelId: -2 }
    ])
    expect(h.driver.sentOf('changeRealm')).toEqual([])
    expect(h.session().travellingTo).toBeNull()
  })

  it('random is a Genesis City parcel from a realm of any kind', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    // the middle of the city
    const random = vi.spyOn(Math, 'random').mockReturnValue(0.5)
    try {
      inGenesis(h)
      act(() => h.session().chat.send('/goto random'))
      expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', x: 0, y: 0, travelId: -1 }])

      // from a preview it is a realm change, which the engine refuses there in its own words
      inPreview(h)
      act(() => h.session().chat.send('/goto random'))
      expect(h.driver.sentOf('teleport')[1]).toEqual({ kind: 'teleport', realm: DEFAULT_REALM, x: 0, y: 0, travelId: 2 })
      act(() => h.driver.emit({ kind: 'travelResult', travelId: 2, realm: DEFAULT_REALM, ok: false, message: 'Changing realm is disabled in preview mode.' }))
      expect(systemLines(h).at(-1)).toBe('🔴 Error: Changing realm is disabled in preview mode.')
    } finally {
      random.mockRestore()
    }
  })

  it('to the world the player is in changes no realm', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inWorld(h, 'https://worlds.example/world/Boedo.dcl.eth')
    act(() => h.session().chat.send('/goto boedo'))
    expect(systemLines(h)).toEqual(['🟡 You are already in boedo!'])
    act(() => h.session().chat.send('/goto boedo/3,4'))
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', x: 3, y: 4, travelId: -1 }])
    expect(h.driver.sentOf('changeRealm')).toEqual([])
  })

  it('the realm travelled to is known from the travel itself, before the engine next reports it', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inGenesis(h)
    act(() => h.session().chat.send('/goto boedo'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 2, realm: 'boedo.dcl.eth', ok: true }))
    // no realmInfo yet: the last one still says Genesis City, whose extension is outside a world
    act(() => h.session().chat.send('/goto 160,155'))
    expect(h.driver.sentOf('teleport')).toEqual([])
    expect(systemLines(h).at(-1)).toBe('🔴 Error: Parcel (160, 155) is outside of the bounds.')
    act(() => h.session().chat.send('/goto genesis/10,20'))
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', realm: DEFAULT_REALM, x: 10, y: 20, travelId: 3 }])
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 3, realm: DEFAULT_REALM, ok: true }))
    // and back in the city that parcel is one of the realm
    act(() => h.session().chat.send('/goto 160,155'))
    expect(h.driver.sentOf('teleport')[1]).toEqual({ kind: 'teleport', x: 160, y: 155, travelId: -1 })
    // the engine's report takes over once it comes
    inWorld(h, 'mana.dcl.eth')
    act(() => h.session().chat.send('/goto mana'))
    expect(systemLines(h).at(-1)).toBe('🟡 You are already in mana!')
  })

  it('typed while a travel is under way is read against the realm travelled to', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inGenesis(h)
    act(() => h.session().chat.send('/goto boedo'))
    // on the way to boedo, Genesis City is a realm change again: it takes the place of boedo's
    act(() => h.session().chat.send('/goto genesis'))
    expect(h.driver.sentOf('changeRealm')).toEqual([{ kind: 'changeRealm', realm: 'boedo.dcl.eth', travelId: 2 }])
    // to Genesis Plaza, as it names no parcel
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', realm: DEFAULT_REALM, x: 0, y: 0, travelId: 3 }])
    expect(systemLines(h)).toEqual([CANCELED])
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 3, realm: DEFAULT_REALM, ok: true }))
    expect(systemLines(h).at(-1)).toBe('🟢 Welcome to Genesis City!')

    // on the way to a world its own parcel is that travel again, and so is a parcel alone
    act(() => h.session().chat.send('/goto mana'))
    act(() => h.session().chat.send('/goto mana/3,4'))
    expect(h.driver.sentOf('teleport')[1]).toEqual({ kind: 'teleport', realm: 'mana.dcl.eth', x: 3, y: 4, travelId: 5 })
    act(() => h.session().chat.send('/goto 10,20'))
    expect(h.driver.sentOf('teleport')[2]).toEqual({ kind: 'teleport', realm: 'mana.dcl.eth', x: 10, y: 20, travelId: 6 })
  })

  it('to a place inside the realm travelled to is that travel again, so one that fails moves nobody', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inGenesis(h)
    act(() => h.session().chat.send('/goto nosuchworld'))
    act(() => h.session().chat.send('/goto nosuchworld/3,4'))
    // nothing is asked of the realm being left
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', realm: 'nosuchworld.dcl.eth', x: 3, y: 4, travelId: 3 }])
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 3, realm: 'nosuchworld.dcl.eth', ok: false, message: 'status: 404 Not Found' }))
    expect(systemLines(h)).toEqual([CANCELED, "🔴 Error: The world nosuchworld doesn't exist or not reachable!"])

    // /goto-local too: it answers as a teleport once the realm is entered
    act(() => h.session().chat.send('/goto mana'))
    act(() => h.session().chat.send('/goto-local 5,6'))
    expect(h.driver.sentOf('teleport')[1]).toEqual({ kind: 'teleport', realm: 'mana.dcl.eth', x: 5, y: 6, travelId: 5 })
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 5, realm: 'mana.dcl.eth', ok: true }))
    expect(systemLines(h).at(-1)).toBe('🟢 You teleported to 5,6.')
  })

  it('on the way to an address that tells no kind, the realm is of no known kind', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inGenesis(h)
    act(() => h.session().chat.send('/goto https://other.example/main'))
    act(() => h.session().chat.send('/goto genesis'))
    expect(h.driver.sentOf('changeRealm')).toEqual([{ kind: 'changeRealm', realm: 'https://other.example/main', travelId: 2 }])
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', realm: DEFAULT_REALM, x: 0, y: 0, travelId: 3 }])
  })

  it('a travel to an address that tells no kind leaves the realm to the engine to report', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inGenesis(h)
    // Genesis City by another address: its report already came, and no other follows
    act(() => h.session().chat.send('/goto https://other.example/main'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 2, realm: 'https://other.example/main', ok: true }))
    act(() => h.session().chat.send('/goto genesis'))
    expect(systemLines(h).at(-1)).toBe('🟡 You are already in genesis!')
    act(() => h.session().chat.send('/goto 200,200'))
    expect(systemLines(h).at(-1)).toBe('🔴 Error: Parcel (200, 200) is outside of the bounds.')
  })

  it('crowd answered after a sign-out neither travels nor posts', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    let answer: (response: Response) => void = () => undefined
    const fetched = vi.spyOn(globalThis, 'fetch').mockReturnValue(new Promise<Response>((resolve) => (answer = resolve)))
    try {
      act(() => h.session().chat.send('/goto crowd'))
      const before = systemLines(h)
      act(() => h.session().logout())
      await act(async () => {
        answer(new Response(JSON.stringify([{ baseCoords: [5, 6] }])))
        await Promise.resolve()
      })
      expect(h.driver.sentOf('teleport')).toEqual([])
      expect(systemLines(h)).toEqual(before)
    } finally {
      fetched.mockRestore()
    }
  })

  it('crowd answered after a later /goto gives way to it', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    let answer: (response: Response) => void = () => undefined
    let refuse: (reason: Error) => void = () => undefined
    const fetched = vi.spyOn(globalThis, 'fetch').mockImplementation(
      () =>
        new Promise<Response>((resolve, reject) => {
          answer = resolve
          refuse = reject
        })
    )
    // past every step of the read
    const settled = (): Promise<void> => act(async () => new Promise<void>((resolve) => setTimeout(resolve)))
    try {
      act(() => h.session().chat.send('/goto crowd'))
      act(() => h.session().chat.send('/goto boedo'))
      answer(new Response(JSON.stringify([{ baseCoords: [5, 6] }])))
      await settled()
      expect(h.driver.sentOf('teleport')).toEqual([])
      expect(h.driver.sentOf('changeRealm')).toEqual([{ kind: 'changeRealm', realm: 'boedo.dcl.eth', travelId: 2 }])
      expect(h.session().travellingTo).toBe('boedo.dcl.eth')
      expect(systemLines(h)).toEqual([])

      // one that cannot be read either: the later command has nothing to hear of it
      vi.spyOn(console, 'warn').mockImplementation(() => undefined)
      act(() => h.session().chat.send('/goto crowd'))
      act(() => h.session().chat.send('/goto 200,200'))
      const lines = systemLines(h)
      refuse(new Error('offline'))
      await settled()
      expect(systemLines(h)).toEqual(lines)
    } finally {
      fetched.mockRestore()
    }
  })

  it('crowd says so when the busiest scene cannot be read', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const fetched = vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('offline'))
    try {
      act(() => h.session().chat.send('/goto crowd'))
      await waitFor(() => expect(systemLines(h)).toEqual(['🔴 Error running command.']))
      expect(h.driver.sentOf('teleport')).toEqual([])
    } finally {
      fetched.mockRestore()
    }
  })

  it('that a later travel replaced says it was canceled, once', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().chat.send('/goto first'))
    act(() => h.session().chat.send('/goto second'))
    expect(systemLines(h)).toEqual([CANCELED])
    // the engine's own answer for the one replaced adds nothing
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 2, realm: 'first.dcl.eth', ok: false, message: 'superseded by a later realm change' }))
    expect(systemLines(h)).toEqual([CANCELED])
    // the later one still travels
    expect(h.session().travellingTo).toBe('second.dcl.eth')

    // a travel asked for outside chat replaces it the same way
    act(() => h.session().map.changeRealm('third.dcl.eth'))
    expect(systemLines(h)).toEqual([CANCELED, CANCELED])
  })

  it('that a travel the session did not ask for replaced says it was canceled', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().chat.send('/goto first'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 2, realm: 'first.dcl.eth', ok: false, message: 'superseded by a later realm change' }))
    expect(systemLines(h)).toEqual([CANCELED])
  })

  it('inside the realm whose answer never came gives way to the next, with no line of its own', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inGenesis(h)
    act(() => h.session().chat.send('/goto 1,2'))
    act(() => h.session().chat.send('/goto 3,4'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: -1, realm: '', ok: true }))
    expect(systemLines(h)).toEqual([])
    act(() => h.driver.emit({ kind: 'travelResult', travelId: -2, realm: '', ok: true }))
    expect(systemLines(h)).toEqual(['🟢 You teleported to 3,4.'])
  })

  it("refused by a platform ban opens the ban's dialog and posts no line", async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().chat.send('/goto mana'))
    act(() => h.driver.emit({ kind: 'travelResult', travelId: 2, realm: 'mana.dcl.eth', ok: false, message: PLATFORM_BANNED_REFUSAL }))
    expect(systemLines(h)).toEqual([])
    expect(h.session().platformBan.travelRefused).toBe(true)
    expect(h.session().travelError).toBeNull()
    expect(h.session().travellingTo).toBeNull()
  })

  it('answered after a sign-out posts nothing', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().chat.send('/goto-local 1,2'))
    const before = systemLines(h)
    act(() => h.session().logout())
    act(() => h.driver.emit({ kind: 'travelResult', travelId: -1, realm: '', ok: true }))
    expect(systemLines(h)).toEqual(before)
  })
})

describe("chat's /goto-local", () => {
  it('stays in the realm the player is in', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    inWorld(h, 'boedo.dcl.eth')
    act(() => h.session().chat.send('/goto-local 1,2'))
    expect(h.driver.sentOf('teleport')).toEqual([{ kind: 'teleport', x: 1, y: 2, travelId: -1 }])
    expect(h.driver.sentOf('changeRealm')).toEqual([])
    act(() => h.driver.emit({ kind: 'travelResult', travelId: -1, realm: '', ok: true }))
    expect(systemLines(h)).toEqual(['🟢 You teleported to 1,2.'])
    act(() => h.session().chat.send('/goto-local random'))
    expect(systemLines(h).at(-1)).toContain('🔴 Invalid parameters, usage:')
  })
})
