import { describe, it, expect, vi } from 'vitest'
import type { PageToScene } from '@explorer-core/engine/protocol'

const setChatBubbleText = vi.hoisted(() => vi.fn())
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { setChatBubbleText } }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/domains/profile', () => ({ httpOrUndef: vi.fn(), profileKey: vi.fn() }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({ engine: {}, PlayerIdentityData: {}, PointerLock: {} }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: vi.fn() }))
vi.mock('~system/Players', () => ({ getPlayerData: vi.fn() }))

describe('bridge scene: the chat domain', () => {
  it("replaces a chat bubble's text through the engine's typed call", async () => {
    const { registerChat } = await import('../../bridge-scene/src/domains/chat')
    const handlers = new Map<string, (msg: PageToScene) => void>()
    registerChat({ send: vi.fn(), on: (kind, handler) => handlers.set(kind, handler as never), push: vi.fn() })

    handlers.get('chatBubbleText')!({ kind: 'chatBubbleText', sender: '0x1', messageId: 'm1', message: 'hola' })

    expect(setChatBubbleText.mock.calls).toEqual([['m1', 'hola']])
  })
})
