import { describe, it, expect, vi } from 'vitest'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

const setChatBubbleText = vi.hoisted(() => vi.fn())
const getAvatarModifiers = vi.hoisted(() => vi.fn())
const getEntitiesWith = vi.hoisted(() => vi.fn())
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

  it('shares modifier polling with profile picking and keeps Nearby current during RPC failures', async () => {
    const { registerAvatarModifiers } = await import('../../bridge-scene/src/avatar-modifiers')
    const { registerAvatarPointer } = await import('../../bridge-scene/src/domains/avatarPointer')
    const { registerChat } = await import('../../bridge-scene/src/domains/chat')
    // LiveKit keeps both identities in the SDK view through the Pulse departure.
    getEntitiesWith.mockReturnValue([[1, { address: '0xA' }], [2, { address: '0xB' }]])
    getAvatarModifiers.mockReset().mockResolvedValueOnce([])
    const systems: Array<(dt: number) => void> = []
    const rosters: string[][] = []
    const send = vi.fn((msg: SceneToPage) => {
      if (msg.kind === 'members') rosters.push(msg.members.map((member) => member.address))
    })
    const ctx = { send, on: vi.fn(), push: (system: (dt: number) => void) => systems.push(system) }
    const avatarModifiers = registerAvatarModifiers(ctx)
    registerChat(ctx, avatarModifiers)
    registerAvatarPointer(ctx, avatarModifiers)

    const tick = (dt: number) => {
      for (const system of systems) system(dt)
    }
    const initial = avatarModifiers()
    tick(0.016)
    expect(getAvatarModifiers).toHaveBeenCalledTimes(1)
    expect(rosters).toEqual([['0xA', '0xB']])
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

    getAvatarModifiers.mockRejectedValueOnce(new Error('modifier snapshot unavailable'))
    tick(1.5)
    expect(rosters.at(-1)).toEqual(['0xB'])
    await expect(getAvatarModifiers.mock.results[2].value).rejects.toThrow('modifier snapshot unavailable')

    getEntitiesWith.mockReturnValue([[1, { address: '0xA' }], [3, { address: '0xC' }]])
    getAvatarModifiers.mockRejectedValueOnce(new Error('modifier snapshot still unavailable'))
    tick(3)
    expect(rosters.at(-1)).toEqual(['0xC'])
    await expect(getAvatarModifiers.mock.results[3].value).rejects.toThrow('modifier snapshot still unavailable')

    getAvatarModifiers.mockResolvedValueOnce([])
    tick(1)
    await vi.waitFor(() => expect(avatarModifiers().pulseAbsent.size).toBe(0))
    tick(2)
    expect(rosters.at(-1)).toEqual(['0xA', '0xC'])
  })
})
