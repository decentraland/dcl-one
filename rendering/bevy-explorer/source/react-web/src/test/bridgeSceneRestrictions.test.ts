// The bridge scene's scene restrictions relay: what the engine's stream reports reaches the page,
// and a page that starts late is sent the last report again.
import { beforeEach, describe, it, expect, vi } from 'vitest'
import type { SceneRestrictions } from '@explorer-core/engine/generated'

const { relay } = vi.hoisted(() => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay }))
const api = vi.hoisted(() => ({ getSceneRestrictionsStream: vi.fn() as ReturnType<typeof vi.fn> | undefined }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: api }))

const { registerSceneRestrictions } = await import('../../bridge-scene/src/domains/sceneRestrictions')

const NONE: SceneRestrictions = {
  cameraLocked: false,
  avatarHidden: false,
  avatarMovementsBlocked: false,
  passportCannotBeOpened: false,
  experiencesBlocked: false,
  skyboxTimeUiBlocked: false,
  nearbyVoiceChatBlocked: false
}
const CAMERA: SceneRestrictions = { ...NONE, cameraLocked: true }

function bridge(): { sent: unknown[]; on: Map<string, () => void> } {
  const sent: unknown[] = []
  const on = new Map<string, () => void>()
  registerSceneRestrictions({
    send: (msg) => sent.push(msg),
    on: (kind, handler) => on.set(kind, handler as () => void),
    push: () => undefined
  })
  return { sent, on }
}

describe('bridge scene restrictions', () => {
  beforeEach(() => {
    relay.mockClear()
    api.getSceneRestrictionsStream = vi.fn()
  })

  it('relays each report of the engine stream', async () => {
    const b = bridge()
    const [name, open, onItem] = relay.mock.calls[0] as [string, () => Promise<unknown>, (state: SceneRestrictions) => void]
    expect(name).toBe('sceneRestrictions')
    await open()
    expect(api.getSceneRestrictionsStream).toHaveBeenCalledTimes(1)
    onItem(CAMERA)
    onItem(NONE)
    expect(b.sent).toEqual([
      { kind: 'sceneRestrictions', state: CAMERA },
      { kind: 'sceneRestrictions', state: NONE }
    ])
  })

  it('sends the last report again to a page that starts late', () => {
    const b = bridge()
    const onItem = relay.mock.calls[0][2] as (state: SceneRestrictions) => void
    onItem(CAMERA)
    b.on.get('hello')?.()
    expect(b.sent).toEqual([
      { kind: 'sceneRestrictions', state: CAMERA },
      { kind: 'sceneRestrictions', state: CAMERA }
    ])
  })

  it('sends nothing to a page that starts before the first report', () => {
    const b = bridge()
    b.on.get('hello')?.()
    expect(b.sent).toEqual([])
  })

  it('registers nothing on an engine without the stream', () => {
    api.getSceneRestrictionsStream = undefined
    const b = bridge()
    expect(relay).not.toHaveBeenCalled()
    expect(b.on.size).toBe(0)
  })
})
