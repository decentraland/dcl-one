import { describe, it, expect, vi } from 'vitest'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

const setChatBubbleText = vi.hoisted(() => vi.fn())
const getAvatarModifiers = vi.hoisted(() => vi.fn())
const getEntitiesWith = vi.hoisted(() => vi.fn())
const getTransform = vi.hoisted(() => vi.fn())
const onPointerDown = vi.hoisted(() => vi.fn())
const removeOnPointerDown = vi.hoisted(() => vi.fn())
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { setChatBubbleText, getAvatarModifiers } }))
vi.mock('../../bridge-scene/src/system-helpers', async (importOriginal) => ({
  ...await importOriginal<typeof import('../../bridge-scene/src/system-helpers')>(),
  relay: vi.fn()
}))
vi.mock('../../bridge-scene/src/domains/profile', () => ({ httpOrUndef: vi.fn(), profileKey: (address: string) => address.toLowerCase() }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({
  engine: { getEntitiesWith },
  PlayerIdentityData: { getOrNull: () => ({ address: '0xB' }) },
  Transform: { getOrNull: getTransform },
  PointerLock: {},
  pointerEventsSystem: { onPointerDown, removeOnPointerDown },
  InputAction: { IA_POINTER: 1 }
}))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: vi.fn() }))
vi.mock('~system/Players', () => ({ getPlayerData: vi.fn().mockResolvedValue({}) }))

describe('bridge scene: the chat domain', () => {
  it("replaces a chat bubble's text through the engine's typed call", async () => {
    const { registerChat } = await import('../../bridge-scene/src/domains/chat')
    const handlers = new Map<string, (msg: PageToScene) => void>()
    registerChat(
      { send: vi.fn(), on: (kind, handler) => handlers.set(kind, handler as never), push: vi.fn() },
      () => ({ passportDisabled: new Set(), pulseAbsent: new Set() })
    )

    handlers.get('chatBubbleText')!({ kind: 'chatBubbleText', sender: '0x1', messageId: 'm1', message: 'hola' })

    expect(setChatBubbleText.mock.calls).toEqual([['m1', 'hola']])
  })

  it('shares modifier polling across profile picking, Nearby and the map', async () => {
    const { registerAvatarModifiers } = await import('../../bridge-scene/src/domains/avatarModifiers')
    const { registerAvatarPointer } = await import('../../bridge-scene/src/domains/avatarPointer')
    const { registerChat } = await import('../../bridge-scene/src/domains/chat')
    const { registerMapPlayers } = await import('../../bridge-scene/src/domains/mapPlayers')
    // LiveKit keeps both identities in the SDK view through the Pulse departure.
    getEntitiesWith.mockReturnValue([[1, { address: '0xA' }], [2, { address: '0xB' }]])
    getTransform.mockImplementation((entity: number) => ({ position: { x: entity * 10, z: 0 } }))
    getAvatarModifiers.mockReset().mockResolvedValueOnce([])
    const systems: Array<(dt: number) => void> = []
    const rosters: string[][] = []
    const mapRosters: string[][] = []
    const send = vi.fn((msg: SceneToPage) => {
      if (msg.kind === 'members') rosters.push(msg.members.map((member) => member.address))
      if (msg.kind === 'mapPlayers') mapRosters.push(msg.players.map((player) => player.address))
    })
    const ctx = { send, on: vi.fn(), push: (system: (dt: number) => void) => systems.push(system) }
    const avatarModifiers = registerAvatarModifiers(ctx)
    registerChat(ctx, avatarModifiers)
    registerAvatarPointer(ctx, avatarModifiers)
    registerMapPlayers(ctx, avatarModifiers)
    ctx.on.mock.calls.find(([kind]) => kind === 'watchMapPlayers')![1]({ kind: 'watchMapPlayers', on: true })

    const tick = (dt: number) => {
      for (const system of systems) system(dt)
    }
    const initial = avatarModifiers()
    tick(0.016)
    expect(getAvatarModifiers).toHaveBeenCalledTimes(1)
    expect(rosters).toEqual([['0xA', '0xB']])
    expect(mapRosters).toEqual([['0xA', '0xB']])
    await vi.waitFor(() => expect(avatarModifiers()).not.toBe(initial))

    getAvatarModifiers.mockResolvedValueOnce([
      { userId: '0xa', hideAvatar: false, hideProfile: false, pulseAbsent: true },
      { userId: '0xb', hideAvatar: true, hideProfile: false, pulseAbsent: false }
    ])
    tick(1)
    expect(getAvatarModifiers).toHaveBeenCalledTimes(2)
    await vi.waitFor(() => expect(avatarModifiers().pulseAbsent.has('0xa')).toBe(true))
    const clickHidden = onPointerDown.mock.calls.find(([target]) => target.entity === 2)![1]
    clickHidden()
    expect(send).not.toHaveBeenCalledWith({ kind: 'avatarClick', address: '0xB' })
    tick(0.5)
    expect(removeOnPointerDown).toHaveBeenCalledWith(2)
    expect(mapRosters.at(-1)).toEqual(['0xB'])

    getAvatarModifiers.mockRejectedValueOnce(new Error('modifier snapshot unavailable'))
    tick(1.5)
    expect(rosters.at(-1)).toEqual(['0xB'])
    await new Promise((resolve) => setTimeout(resolve, 0))

    getEntitiesWith.mockReturnValue([[1, { address: '0xA' }], [3, { address: '0xC' }]])
    getAvatarModifiers.mockRejectedValueOnce(new Error('modifier snapshot still unavailable'))
    tick(3)
    expect(rosters.at(-1)).toEqual(['0xC'])
    expect(mapRosters.at(-1)).toEqual(['0xC'])
    await new Promise((resolve) => setTimeout(resolve, 0))

    getAvatarModifiers.mockResolvedValueOnce([])
    getTransform.mockImplementation((entity: number) => ({ position: { x: entity * 20, z: 0 } }))
    tick(1)
    await vi.waitFor(() => expect(avatarModifiers().pulseAbsent.size).toBe(0))
    tick(2)
    expect(rosters.at(-1)).toEqual(['0xA', '0xC'])
    expect(mapRosters.at(-1)).toEqual(['0xA', '0xC'])
    expect(send).toHaveBeenLastCalledWith({
      kind: 'mapPlayers',
      players: [{ address: '0xA', x: 20, z: 0 }, { address: '0xC', x: 60, z: 0 }]
    })
  })
})
