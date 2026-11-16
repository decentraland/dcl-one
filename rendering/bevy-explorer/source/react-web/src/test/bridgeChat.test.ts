import { describe, it, expect, vi } from 'vitest'
import type { PageToScene } from '@explorer-core/engine/protocol'

const consoleCommand = vi.hoisted(() => vi.fn(async () => ''))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { consoleCommand } }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/domains/profile', () => ({ httpOrUndef: vi.fn(), profileKey: vi.fn() }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({ engine: {}, PlayerIdentityData: {}, PointerLock: {} }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: vi.fn() }))
vi.mock('~system/Players', () => ({ getPlayerData: vi.fn() }))

describe('bridge scene: the chat domain', () => {
  it("replaces a chat bubble's text through the engine's chat_bubble_text command", async () => {
    const { registerChat } = await import('../../bridge-scene/src/domains/chat')
    const handlers = new Map<string, (msg: PageToScene) => void>()
    registerChat({ send: vi.fn(), on: (kind, handler) => handlers.set(kind, handler as never), push: vi.fn() })

    handlers.get('chatBubbleText')!({ kind: 'chatBubbleText', sender: '0x1', messageId: 'm1', message: 'hola' })

    // the engine's console op adds the leading slash itself
    expect(consoleCommand.mock.calls).toEqual([['chat_bubble_text', ['m1', 'hola']]])
  })
})
