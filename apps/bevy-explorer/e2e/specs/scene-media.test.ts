// QA checklist media cases: `Video Streaming` NFT AND GIFS (2) and `General Functionality`
// GENESIS PLAZA Theatre (1).
//
// | Checklist case                  | Here                                                         | Class   |
// | Looping NFTs (14,100)           | the scene runs and has NFT frames (`NftShape` entities)      | Partial |
// | Static NFTs (80,-4)             | the scene runs and has NFT frames (`NftShape` entities)      | Partial |
// | Theatre (Genesis Plaza)         | a video player in Genesis Plaza is requested and scheduled   | Partial |
//
// Partial:
// - The NFT cases check that the scene declares its frames; that an animated NFT loops, and that
//   each frame shows its image, is what the eye checks (the final frame is in the report).
// - Theatre: the checklist's placeholder video, the stage lights and the emotes. Here the player
//   walks from the scene's spawn to the seats in front of the screen; the scene then creates the
//   theatre's video, and `/media_state` shows it requested to play and scheduled by the engine.
//   The lights and the emotes are not checked.
//
// Like `specs/sdk-scenes.test.ts`, these scenes are deployed on mainnet only, so the scenarios run
// on the client that spec boots into `DCL_E2E_GENESIS_REALM` (`https://peer.decentraland.org`).
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type ScenarioOptions } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const GENESIS_REALM = process.env.DCL_E2E_GENESIS_REALM || 'https://peer.decentraland.org'
/** The same session as `sdk-scenes`: one client for both. */
const GENESIS_SESSION: ScenarioOptions['session'] = { realm: GENESIS_REALM, position: '0,0' }

const ARRIVAL_TIMEOUT_MS = 90_000
const LOAD_TIMEOUT_MS = 60_000
/** How long a running scene has to create its entities, or to start its video. */
const CONTENT_TIMEOUT_MS = 30_000
const SCENARIO_TIMEOUT_MS = 300_000
/** In front of the theatre's screen, which stands at (65, 12, 118) from Genesis Plaza's base
 *  parcel -3,-2: at world (17, 86). A teleport into the scene lands on its spawn point, so the
 *  player walks there; the scene starts the theatre's video for a player at the seats. */
const THEATRE_SEATS: [number, number, number] = [17, 0, 66]
const WALK_TIMEOUT_S = 60
const OPTIONS: ScenarioOptions = { session: GENESIS_SESSION, timeoutMs: SCENARIO_TIMEOUT_MS }

interface MediaPlayer {
  type: 'video' | 'audio'
  entity: string
  scene: string | null
  inScene: boolean
  requested: boolean
  scheduled: boolean
  source?: string
}

interface MediaState {
  backend: string
  video: number
  audio: number
  players: MediaPlayer[]
}

/** Teleport to the parcel and wait for its scene to run. */
async function enterScene(explorer: Explorer, x: number, y: number): Promise<string> {
  await explorer.teleport(x, y, { timeoutMs: ARRIVAL_TIMEOUT_MS })
  const loaded = await until(
    async () => JSON.parse(await explorer.run('/scene_loading_state')) as { stage: string },
    (state) => ['Running', 'Broken', 'Failed'].includes(state.stage),
    { timeoutMs: LOAD_TIMEOUT_MS, intervalMs: 1000, what: `the scene at ${x},${y} to run` }
  )
  assert.equal(loaded.stage, 'Running', `the scene at ${x},${y} should run, not end ${loaded.stage}`)
  const info = JSON.parse(await explorer.run('/current_scene_state')) as { name?: string }
  return info.name ?? '?'
}

async function nftFrames(explorer: Explorer, x: number, y: number): Promise<string[]> {
  return until(() => explorer.sceneEntities('NftShape'), (entities) => entities.length > 0, {
    timeoutMs: CONTENT_TIMEOUT_MS,
    intervalMs: 1000,
    what: `the scene at ${x},${y} to have NFT frames (NftShape entities)`
  })
}

scenario(
  'the NFT gallery at 14,100 has its NFT frames',
  async ({ explorer, t }) => {
    t.diagnostic(`scene "${await enterScene(explorer, 14, 100)}"`)
    const frames = await nftFrames(explorer, 14, 100)
    t.diagnostic(`${frames.length} NftShape entities`)
  },
  OPTIONS
)

scenario(
  'the corridor at 80,-4 has its NFT frames',
  async ({ explorer, t }) => {
    t.diagnostic(`scene "${await enterScene(explorer, 80, -4)}"`)
    const frames = await nftFrames(explorer, 80, -4)
    t.diagnostic(`${frames.length} NftShape entities`)
  },
  OPTIONS
)

scenario(
  "Genesis Plaza's theatre plays its video",
  async ({ explorer, t }) => {
    t.diagnostic(`scene "${await enterScene(explorer, 0, 0)}"`)
    await explorer.walkTo(...THEATRE_SEATS, WALK_TIMEOUT_S)
    t.diagnostic(`the player stands at ${JSON.stringify((await explorer.worldState()).position)}`)
    const media = await until(
      async () => JSON.parse(await explorer.run('/media_state')) as MediaState,
      (state) => state.players.some((player) => player.type === 'video' && player.requested && player.scheduled),
      { timeoutMs: CONTENT_TIMEOUT_MS, intervalMs: 1000, what: 'a Genesis Plaza video that is requested to play and scheduled' }
    )
    for (const player of media.players) t.diagnostic(`${player.type} ${player.entity} ${player.source ?? ''} requested=${player.requested} scheduled=${player.scheduled}`)
    const playing = media.players.filter((player) => player.type === 'video' && player.requested && player.scheduled)
    assert.ok(playing.length > 0, 'a video of the scene should play')
  },
  OPTIONS
)
