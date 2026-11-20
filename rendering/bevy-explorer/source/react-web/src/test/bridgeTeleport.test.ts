import { describe, it, expect, vi } from 'vitest'
import { teleport } from '../../bridge-scene/src/domains/teleport'

// The bridge's teleport (bridge-scene/src/domains/teleport): every HUD teleport goes through the
// engine's teleport for the HUD, and the scene API's teleportTo only where the engine lacks it.
describe("the bridge's teleport", () => {
  it("goes through the engine's teleport for the HUD, with or without a spawn point", async () => {
    const hud = vi.fn(async () => undefined)
    const scene = vi.fn(async () => ({}))
    await teleport({ parcel: { x: 1, y: 2 } }, hud, scene)
    await teleport({ realm: 'mana.dcl.eth', spawnPoint: 'door' }, hud, scene)
    expect(hud.mock.calls).toEqual([[{ parcel: { x: 1, y: 2 } }], [{ realm: 'mana.dcl.eth', spawnPoint: 'door' }]])
    expect(scene).not.toHaveBeenCalled()
  })

  it("passes on the engine's refusal", async () => {
    const hud = vi.fn(async () => {
      throw new Error('status: 404 Not Found')
    })
    await expect(teleport({ realm: 'nowhere.dcl.eth' }, hud, vi.fn())).rejects.toThrow('404')
  })

  it('falls back to teleportTo on an engine without it', async () => {
    const scene = vi.fn(async () => ({}))
    await teleport({ parcel: { x: 1, y: 2 }, realm: 'mana.dcl.eth' }, undefined, scene)
    expect(scene).toHaveBeenCalledWith({ worldCoordinates: { x: 1, y: 2 }, realm: 'mana.dcl.eth' })
  })

  it('refuses a named spawn point there, where it could not be honoured', async () => {
    const scene = vi.fn(async () => ({}))
    await expect(teleport({ parcel: { x: 1, y: 2 }, spawnPoint: 'door' }, undefined, scene)).rejects.toThrow('named spawn point')
    expect(scene).not.toHaveBeenCalled()
  })
})
