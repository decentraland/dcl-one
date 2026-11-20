// The scene room reports admission and moderation lists. A scene ban unloads the parcel scene
// while the player remains in the world. A later successful admission restores the scene.
//
// | Scenario                                                   | Class            |
// | a scene room reports its admission and moderation lists    | New              |
// | leaving the scene clears the room state                    | New              |
// | a refused scene stays unloaded across departure and return | New, native only |
//
// The refusal scenario starts its own standalone against a loopback gatekeeper stub
// (`--comms-gatekeeper`); on the web a service override stops the boot at the launch gate.
import assert from 'node:assert/strict'
import { before } from 'node:test'
import { readTarget, refusal, sharedGatekeeperStub, spec, until, type GatekeeperStub } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const ROOM_TIMEOUT_MS = 90_000
const STUB_PORT = Number(process.env.DCL_E2E_GATEKEEPER_PORT ?? 18745)
const native = readTarget().transport === 'native'
/** Genesis Plaza: a scene with a room, where every run lands. */
const SCENE_PARCEL: [number, number] = [0, 0]
/** An empty parcel far from any scene room. */
const EMPTY_PARCEL: [number, number] = [-130, 130]

let stub: GatekeeperStub | undefined
before(async () => {
  if (native) stub = await sharedGatekeeperStub(STUB_PORT)
})

scenario('a scene room reports its admission and moderation lists', async ({ explorer }) => {
  await explorer.teleport(...SCENE_PARCEL)
  const state = await until(() => explorer.banBlockState(), (s) => s.sceneRoom.state === 'connected', {
    timeoutMs: ROOM_TIMEOUT_MS,
    what: 'the scene room to connect'
  })
  assert.equal(state.sceneAdmission.state, 'allowed')
  assert.ok(state.sceneRoom.sceneId)
  assert.equal(state.sceneAdmission.sceneId, state.sceneRoom.sceneId)
  assert.ok(Array.isArray(state.sceneRoom.bannedAddresses))
  assert.ok(Array.isArray(state.sceneRoom.sceneAdmins))
  const lookup = await explorer.banBlockState('0x' + '1'.repeat(40))
  assert.equal(lookup.lookup?.bannedInScene, false)
  assert.notEqual(lookup.lookup?.adminPermissions, 'not_loaded_yet')
})

scenario('leaving the scene clears the room state', async ({ explorer }) => {
  await explorer.teleport(...EMPTY_PARCEL)
  const state = await until(() => explorer.banBlockState(), (s) => s.sceneRoom.state === 'not_connected', {
    timeoutMs: ROOM_TIMEOUT_MS,
    what: 'the scene room to be gone'
  })
  assert.notEqual(state.sceneAdmission.state, 'refused')
  assert.equal(state.unavailable['sceneRoom.sceneId'], 'no scene room connected')
  const lookup = await explorer.banBlockState('0x' + '1'.repeat(40))
  assert.equal(lookup.lookup?.adminPermissions, 'not_loaded_yet')
})

scenario(
  'a refused scene unloads until the player is admitted again',
  async ({ explorer }) => {
    assert.ok(stub)
    await explorer.teleport(...SCENE_PARCEL)
    const loadingState = () => explorer.json<{ stage: string }>('/scene_loading_state')
    await until(loadingState, (s) => s.stage === 'Running', {
      timeoutMs: ROOM_TIMEOUT_MS, what: 'the initial scene to load'
    })
    const target = await explorer.json<{ parcels: string[] }>('/scene_target')
    const occupied = new Set(target.parcels)
    const neighbours = target.parcels.flatMap((parcel) => {
      const [x, y] = parcel.split(',').map(Number) as [number, number]
      return [[x + 1, y], [x - 1, y], [x, y + 1], [x, y - 1]] as [number, number][]
    }).filter((parcel) => !occupied.has(parcel.join(',')))
    neighbours.sort((a, b) => Math.abs(a[0]) + Math.abs(a[1]) - Math.abs(b[0]) - Math.abs(b[1]))
    const neighbour = neighbours[0]
    assert.ok(neighbour, 'the scene has a neighbouring parcel')
    await explorer.teleport(...EMPTY_PARCEL)
    stub.sceneAdapter(refusal(403, 'User is banned from this scene'))
    try {
      await explorer.teleport(...SCENE_PARCEL)
      const state = await until(() => explorer.banBlockState(), (s) => s.sceneAdmission.state === 'refused', {
        timeoutMs: ROOM_TIMEOUT_MS,
        what: 'the refusal to be reported'
      })
      assert.equal(state.sceneAdmission.scope, 'scene')
      assert.equal(state.sceneAdmission.message, 'User is banned from this scene')
      assert.equal(state.sceneRoom.state, 'not_connected')

      const absent = 'Waiting for the parcel scene to load.'
      await until(loadingState, (s) => s.stage === absent, {
        timeoutMs: ROOM_TIMEOUT_MS,
        what: 'the refused scene to unload'
      })
      assert.ok((await explorer.worldState()).inWorld)
      await explorer.teleport(...neighbour)
      await until(() => explorer.banBlockState(), (s) => s.sceneAdmission.sceneId !== state.sceneAdmission.sceneId, {
        timeoutMs: ROOM_TIMEOUT_MS, what: 'the room target to leave the refused scene'
      })
      await explorer.teleport(...SCENE_PARCEL)
      await until(() => explorer.banBlockState(), (s) =>
        s.sceneAdmission.state === 'refused' && s.sceneAdmission.sceneId === state.sceneAdmission.sceneId, {
          timeoutMs: ROOM_TIMEOUT_MS, what: 'the refused parcel to regain its admission owner'
        })
      assert.equal((await loadingState()).stage, absent)

      // Admission succeeds; a loopback endpoint keeps the transport out of this assertion.
      stub.sceneAdapter({ status: 200, body: { adapter: 'livekit:wss://127.0.0.1:9/?access_token=e2e-admission' } })
      await until(() => explorer.banBlockState(), (s) => s.sceneAdmission.state === 'allowed', {
        timeoutMs: ROOM_TIMEOUT_MS,
        what: 'a fresh admission to lift the scene ban'
      })
      await until(loadingState, (s) => s.stage === 'Running', {
        timeoutMs: ROOM_TIMEOUT_MS,
        what: 'the scene to load again on the same parcel'
      })
    } finally {
      stub.sceneAdapter(refusal(503, 'stub has no adapter', { 'retry-after': '1' }))
    }
  },
  native ? { session: { nativeArgs: ['--comms-gatekeeper', `http://127.0.0.1:${STUB_PORT}`] } } : { todo: 'web: a loopback service override stops at the launch gate' }
)
