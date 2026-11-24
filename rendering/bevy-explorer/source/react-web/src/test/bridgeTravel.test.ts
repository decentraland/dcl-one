import { beforeEach, describe, it, expect, vi } from 'vitest'

// The world domain's travel handlers (bridge-scene/src/domains/world): what the page's teleport and
// changeRealm ask of the scene API's teleportTo, and the travelResult each is answered with.
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({ engine: {}, Transform: { getOrNull: () => null } }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => null }))
const teleportTo = vi.hoisted(() => vi.fn())
vi.mock('~system/RestrictedActions', () => ({ teleportTo }))
vi.mock('~system/Runtime', () => ({ getRealm: vi.fn(async () => ({})) }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: {} }))

const { registerWorld } = await import('../../bridge-scene/src/domains/world')

type Sent = { kind: string; travelId?: number; realm?: string; ok?: boolean; message?: string }

/** The domain on a fake bridge: what it sent to the page, and its handlers. */
function bridge(): { sent: Sent[]; on: Map<string, (msg?: unknown) => void> } {
  const sent: Sent[] = []
  const on = new Map<string, (msg?: unknown) => void>()
  registerWorld({
    send: (msg) => sent.push(msg as never),
    on: (kind, handler) => on.set(kind, handler as (msg?: unknown) => void),
    push: () => undefined
  })
  return { sent, on }
}

const settled = (): Promise<void> => new Promise((resolve) => setTimeout(resolve))

describe("the bridge's travel handlers", () => {
  beforeEach(() => {
    teleportTo.mockReset()
    teleportTo.mockResolvedValue({})
  })

  it('a teleport inside the realm answers too, with no realm', async () => {
    const b = bridge()
    b.on.get('teleport')?.({ kind: 'teleport', x: 1, y: 2, spawnPoint: 'roof', travelId: -1 })
    expect(teleportTo).toHaveBeenCalledWith({ worldCoordinates: { x: 1, y: 2 }, realm: undefined, spawnPoint: 'roof' })
    await settled()
    expect(b.sent).toEqual([{ kind: 'travelResult', travelId: -1, realm: '', ok: true }])
  })

  it('a teleport to a realm carries its parcel and spawn point, and answers with the realm', async () => {
    const b = bridge()
    b.on.get('teleport')?.({ kind: 'teleport', realm: 'mana.dcl.eth', x: 3, y: 4, spawnPoint: 'door', travelId: 2 })
    expect(teleportTo).toHaveBeenCalledWith({ worldCoordinates: { x: 3, y: 4 }, realm: 'mana.dcl.eth', spawnPoint: 'door' })
    await settled()
    expect(b.sent).toEqual([{ kind: 'travelResult', travelId: 2, realm: 'mana.dcl.eth', ok: true }])
  })

  it('a realm change forwards the spawn point, with no parcel', async () => {
    const b = bridge()
    b.on.get('changeRealm')?.({ kind: 'changeRealm', realm: 'mana.dcl.eth', spawnPoint: 'door', travelId: 3 })
    expect(teleportTo).toHaveBeenCalledWith({ realm: 'mana.dcl.eth', spawnPoint: 'door' })
    await settled()
    expect(b.sent).toEqual([{ kind: 'travelResult', travelId: 3, realm: 'mana.dcl.eth', ok: true }])
  })

  it("a refusal is answered in the engine's words", async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    teleportTo.mockRejectedValue(new Error('status: 404 Not Found'))
    const b = bridge()
    b.on.get('changeRealm')?.({ kind: 'changeRealm', realm: 'nowhere.dcl.eth', travelId: 4 })
    b.on.get('teleport')?.({ kind: 'teleport', x: 1, y: 2, travelId: -2 })
    await settled()
    expect(b.sent).toEqual([
      { kind: 'travelResult', travelId: 4, realm: 'nowhere.dcl.eth', ok: false, message: 'status: 404 Not Found' },
      { kind: 'travelResult', travelId: -2, realm: '', ok: false, message: 'status: 404 Not Found' }
    ])
  })

  it('a request without an id is not answered', async () => {
    const b = bridge()
    b.on.get('teleport')?.({ kind: 'teleport', x: 1, y: 2 })
    await settled()
    expect(teleportTo).toHaveBeenCalledWith({ worldCoordinates: { x: 1, y: 2 }, realm: undefined, spawnPoint: undefined })
    expect(b.sent).toEqual([])
  })
})
