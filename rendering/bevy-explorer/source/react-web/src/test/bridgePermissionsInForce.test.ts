// The bridge scene's permission-used relay, for the reports of what acts on the player now: each
// start and end reaches the page whatever the repeat window, and a page that starts late is sent
// what is still in force.
import { beforeEach, describe, it, expect, vi } from 'vitest'
import type { PermissionUsedRaw } from '../../bridge-scene/src/bevy-api'

const { relay } = vi.hoisted(() => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay }))
vi.mock('~system/Runtime', () => ({ getRealm: async () => ({ realmInfo: { baseUrl: 'https://realm.example' } }) }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: { liveSceneInfo: async () => [{ hash: 'bafkscene', title: 'Plaza' }], getPermissionUsedStream: vi.fn() }
}))

const { registerPermissions } = await import('../../bridge-scene/src/domains/permissions')

type Use = (use: PermissionUsedRaw) => Promise<void>

function bridge(): { sent: unknown[]; on: Map<string, () => void>; use: Use } {
  const sent: unknown[] = []
  const on = new Map<string, () => void>()
  registerPermissions({
    send: (msg) => sent.push(msg),
    on: (kind, handler) => on.set(kind, handler as () => void),
    push: () => undefined
  })
  const used = relay.mock.calls.find(([name]) => name === 'permissionsUsed')
  return { sent, on, use: used?.[2] as Use }
}

const camera = (inForce?: boolean): PermissionUsedRaw => ({ ty: 'ForceCamera', additional: null, scene: 'bafkscene', wasAllowed: true, inForce })
const message = (inForce?: boolean): Record<string, unknown> => ({
  kind: 'permissionUsed',
  ty: 'ForceCamera',
  sceneName: 'Plaza',
  scene: 'bafkscene',
  realm: 'https://realm.example',
  allowed: true,
  ...(inForce == null ? {} : { inForce })
})

describe('bridge permissions in force', () => {
  beforeEach(() => relay.mockClear())

  it('relays every start and end, inside the window that drops a repeated allowed use', async () => {
    const b = bridge()
    await b.use(camera())
    await b.use(camera())
    await b.use(camera(true))
    await b.use(camera(false))
    await b.use(camera(true))
    expect(b.sent).toEqual([message(), message(true), message(false), message(true)])
  })

  it('sends what is still in force again to a page that starts late', async () => {
    const b = bridge()
    await b.use(camera(true))
    await b.use({ ...camera(true), ty: 'SetLocomotion' })
    await b.use(camera(false))
    b.sent.length = 0
    b.on.get('hello')?.()
    expect(b.sent).toEqual([{ ...message(true), ty: 'SetLocomotion' }])
  })

  it('sends nothing on hello while nothing is in force', () => {
    const b = bridge()
    b.on.get('hello')?.()
    expect(b.sent).toEqual([])
  })
})
