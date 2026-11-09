// The scene-ban half of the ban & block status service: the local user's admission to the
// current scene's room and the room's banned and admin lists, as the engine reads them. Not an
// explorer-automation fixture; unity-explorer learns a scene ban from the same refusal
// (`ConnectiveRoom.OnForbiddenAccess`) and the lists from the same room metadata
// (`RoomMetadataCurrentScene`), and what it then does with them is consumer work.
//
// | Scenario                                                   | Class            |
// | a scene room reports its admission and moderation lists    | New              |
// | leaving the scene clears the room state                    | New              |
// | a refused scene adapter marks the player scene-banned      | New, native only |
//
// The refusal scenario starts its own standalone against a loopback gatekeeper stub
// (`--comms-gatekeeper`); on the web a service override stops the boot at the launch gate.
import assert from 'node:assert/strict'
import { after, before } from 'node:test'
import { readTarget, refusal, serveGatekeeperStub, spec, until, type GatekeeperStub } from '../driver/index.ts'

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
  if (native) stub = await serveGatekeeperStub(STUB_PORT)
})
after(async () => {
  await stub?.close()
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
  'a refused scene adapter marks the player scene-banned',
  async ({ explorer }) => {
    assert.ok(stub)
    stub.sceneAdapter(refusal(403, 'User is banned from this scene'))
    await explorer.teleport(...SCENE_PARCEL)
    const state = await until(() => explorer.banBlockState(), (s) => s.sceneAdmission.state === 'refused', {
      timeoutMs: ROOM_TIMEOUT_MS,
      what: 'the refusal to be reported'
    })
    assert.equal(state.sceneAdmission.scope, 'scene')
    assert.equal(state.sceneAdmission.message, 'User is banned from this scene')
    assert.equal(state.sceneRoom.state, 'not_connected')
    // the player is not moved: Unity parity
    const world = await explorer.worldState()
    assert.ok(world.inWorld)
  },
  native ? { session: { nativeArgs: ['--comms-gatekeeper', `http://127.0.0.1:${STUB_PORT}`] } } : { todo: 'web: a loopback service override stops at the launch gate' }
)
