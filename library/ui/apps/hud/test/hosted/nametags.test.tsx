import { describe, it, expect } from 'vitest'
import { renderSession, enterAsGuest } from '../harness'

// DOMAIN: nametags are WORLD-SPACE UI drawn by the engine in 3D (pills over each avatar's head,
// facing the camera), from the profiles, chat and voice state it already holds. They never cross the
// BroadcastChannel, so the React page has no nametag API. This test pins that architectural
// boundary: a full session produces no nametag wire message.
describe('nametags domain (engine-rendered, world-space)', () => {
  it('has no scene↔page API — nothing nametag-related crosses the bridge', async () => {
    const h = renderSession()
    await enterAsGuest(h, { keepSent: true })
    expect(h.driver.sent.some((m) => m.kind.toLowerCase().includes('nametag'))).toBe(false)
  })
})
