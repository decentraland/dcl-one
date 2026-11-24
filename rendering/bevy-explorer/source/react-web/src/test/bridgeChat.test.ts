import { describe, it, expect, vi } from 'vitest'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

const setChatBubbleText = vi.hoisted(() => vi.fn())
const getAvatarModifiers = vi.hoisted(() => vi.fn())
const getEntitiesWith = vi.hoisted(() => vi.fn())
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { setChatBubbleText, getAvatarModifiers } }))
vi.mock('../../bridge-scene/src/system-helpers', async (importOriginal) => ({
  ...await importOriginal<typeof import('../../bridge-scene/src/system-helpers')>(),
  relay: vi.fn()
}))
vi.mock('../../bridge-scene/src/domains/profile', () => ({ httpOrUndef: vi.fn(), profileKey: (address: string) => address.toLowerCase() }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({ engine: { getEntitiesWith }, PlayerIdentityData: {}, PointerLock: {} }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: vi.fn() }))
vi.mock('~system/Players', () => ({ getPlayerData: vi.fn().mockResolvedValue({}) }))

describe('bridge scene: the chat domain', () => {
  it("replaces a chat bubble's text through the engine's typed call", async () => {
    const { registerChat } = await import('../../bridge-scene/src/domains/chat')
    const handlers = new Map<string, (msg: PageToScene) => void>()
    registerChat({ send: vi.fn(), on: (kind, handler) => handlers.set(kind, handler as never), push: vi.fn() })

    handlers.get('chatBubbleText')!({ kind: 'chatBubbleText', sender: '0x1', messageId: 'm1', message: 'hola' })

    expect(setChatBubbleText.mock.calls).toEqual([['m1', 'hola']])
  })

  it('removes a room member from Nearby while Pulse is absent and restores them on rejoin', async () => {
    const { registerChat } = await import('../../bridge-scene/src/domains/chat')
    // LiveKit keeps both identities in the SDK view through the Pulse departure.
    getEntitiesWith.mockReturnValue([[1, { address: '0xA' }], [2, { address: '0xB' }]])
    getAvatarModifiers
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([
        { userId: '0xa', hideAvatar: false, hideProfile: false, pulseAbsent: true },
        { userId: '0xb', hideAvatar: true, hideProfile: false, pulseAbsent: false }
      ])
      .mockResolvedValueOnce([])
    const systems: Array<(dt: number) => void> = []
    const rosters: string[][] = []
    const send = (msg: SceneToPage) => {
      if (msg.kind === 'members') rosters.push(msg.members.map((member) => member.address))
    }
    registerChat({ send, on: vi.fn(), push: (system) => systems.push(system) })

    for (const addresses of [['0xA', '0xB'], ['0xB'], ['0xA', '0xB']]) {
      for (const system of systems) system(3)
      await vi.waitFor(() => expect(rosters.at(-1)).toEqual(addresses))
    }
  })
})
