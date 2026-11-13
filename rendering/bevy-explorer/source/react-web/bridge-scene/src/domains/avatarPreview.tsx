// Avatar preview: the live 3D avatar shown in the React Backpack's left column.
//
// React carves a transparent hole (EngineViewport region='avatarPreview') and sends its
// screen rect over the bridge. We render the player's avatar with a dedicated TextureCamera
// (its own CameraLayer, so no world/skybox/other avatars) into a UI videoTexture positioned
// at that rect — the engine composites it behind the React DOM, showing through the hole.
//
// The preview wears the Backpack's look (./avatarDraft), which equipping edits, so equipping in the
// Backpack reflects here before anything is deployed. Mounted via ReactEcsRenderer in index.ts.
import ReactEcs, { UiEntity } from '@dcl/react-ecs'
import { AssetLoadLoadingState, AvatarShape, CameraLayer, CameraLayers, Material, MeshRenderer, PrimaryPointerInfo, TextureCamera, Transform, engine } from '@dcl/sdk/ecs'
import { Color4, Quaternion, Vector3 } from '@dcl/sdk/math'
import { getPlayer } from '@dcl/sdk/players'
import type { Entity } from '@dcl/ecs'
import type { Ctx } from '../bridge'
import { PREVIEW_MAX_ZOOM, type PreviewFocus } from '@explorer-core/engine/protocol'
import { currentLook } from './avatarDraft'
import type { AvatarLook } from '@explorer-core/engine/avatarEquip'
import { LOBBY_STAGE_PRESETS } from '@explorer-core/engine/lobbyStage'
import { BevyApi } from '../bevy-api'

type Rect = { x: number; y: number; width: number; height: number }

const LAYER = 10
// The Backpack background behind the avatar: the page's left-column purple, top to bottom.
const BACKDROP_TOP = Color4.create(0.31, 0, 0.565, 1)
const BACKDROP_BOTTOM = Color4.create(0.345, 0.078, 0.514, 1)
const BACKDROP_BANDS = Array.from({ length: 24 }, (_, i) => Color4.lerp(BACKDROP_TOP, BACKDROP_BOTTOM, i / 23))
// Podium layers (gold top, orange ring, dark base). The preview's ambient light is 5×, so these
// are about a fifth of the colours they render as.
const PODIUM_LAYERS = [
  { color: Color4.create(0.34, 0.2, 0.02, 1), scale: 0.96, y: -0.06, height: 0.12 },
  { color: Color4.create(0.4, 0.12, 0.02, 1), scale: 1.01, y: -0.1, height: 0.1 },
  { color: Color4.create(0.07, 0.05, 0.09, 1), scale: 1.05, y: -0.16, height: 0.12 }
]
// Camera framing per focus for the 960×960 preview frame (avatar at 2× scale): the visible height
// (orthographic) and the height it's centred on, measured against the reference: whole body with the
// feet at 91% of the frame, head zoomed 2.5× with the eyes at 38%.
const FRAMING: Record<PreviewFocus, { range: number; centerY: number }> = {
  body: { range: 6.25, centerY: 2.54 },
  head: { range: 2.5, centerY: 3.1 },
  top: { range: 3.96, centerY: 2.8 },
  bottom: { range: 3.96, centerY: 1.15 },
  shoes: { range: 3.03, centerY: 0.35 }
}
// The Shop's whole body in its 573×942 window, as the design draws it: the avatar ~751px tall, its feet on
// the stage's floor logo (~970px down the screen). The Shop frames nothing else.
const SHOP_FRAMING = { range: 4.9, centerY: 1.84 }
const FOCUS_SECONDS = 0.6
const CAMERA_PITCH = 12
const CAMERA_DISTANCE = 8
// The camera is pitched down, so its view centre sits this far below it at the avatar.
const PITCH_DROP = CAMERA_DISTANCE * Math.tan((CAMERA_PITCH * Math.PI) / 180)
// Drag-to-rotate sensitivity (from the SDK7 AvatarPreviewElement).
const ROTATION_FACTOR = -0.5
const FACING = Quaternion.fromEulerDegrees(0, 180, 0)
const THUMBNAIL_SIZE = 480

// The lobby stage: the avatar full-screen in the engine's 3D stage (setLobbyStage), camera 1 m
// above a target 0.8 m over the feet, 6 m back, 26.2° vertical field of view.
const LOBBY_TARGET_Y = 0.8
const LOBBY_CAMERA_RISE = 1
const LOBBY_CAMERA_DISTANCE = 6
const LOBBY_FOV = (26.2 * Math.PI) / 180
// The avatar's feet in this scene's coordinates, where the engine builds the stage around it.
const LOBBY_FEET = { x: 8, y: 0, z: 8 }
// The Shop's stage (the HUD's assets/shop/stage.jpg, the same file): drawn behind the avatar where the page
// says it draws it, so the avatar's window in the page shows no seam.
const SHOP_STAGE = 'images/shop-stage.jpg'
// Frames to let the stage and the avatar's first look settle before the page drops its stand-in,
// and how long to wait for an avatar to load at all.
const LOBBY_SETTLE_FRAMES = 45
const LOBBY_LOOK_FRAMES = 600
// The lobby's standby avatar lives on a layer no camera draws: a look loads there out of sight.
const STANDBY_LAYER = 11
// A new look's entrance: the avatar on show turns its back, and the new one, loaded, turns to the
// front from its back, each over this long. Yaws, one way round: 180 faces the camera.
const TURN_SECONDS = 0.5
const YAW_FRONT = 180
const YAW_AWAY = 360
const YAW_BACK = 0
// decentraland.sdk.components.common.LoadingState, as the engine reports an avatar's load on its
// entity's AssetLoadLoadingState (a const enum the SDK does not export at runtime)
const LOADING_NOT_FOUND = 2
const LOADING_FINISHED_WITH_ERROR = 3
const LOADING_FINISHED = 4
// The lobby avatar's idle life: a greeting once it is on show, a flavour emote every so often,
// and a cheer when the player's look changes.
const INTRO_EMOTE = 'wave'
const FLAVOUR_EMOTES = ['wave', 'fistpump', 'dab']
const FLAVOUR_SECONDS: [number, number] = [8, 20]
const LOOK_CHANGED_EMOTE = 'fistpump'

type Stage = 'backpack' | 'lobby'
let stage: Stage = 'backpack'
// The lobby stage's look, as the page last named it, and whether the engine's stage is up.
let stagePreset: string = LOBBY_STAGE_PRESETS[0].id
let stageUp = false
// The lobby avatar's idle emotes are the lobby page's alone (not its stand-ins' pages).
let idleEmotes = false
let flavourIn = 0
let greeted = false

let rect: Rect | null = null
// Frames until a closed region's stage is dropped: a page handing the stage to the next one (the
// guest start page to the lobby, say) closes its region and the next opens its own straight after,
// and the stage, with its loaded avatar, must survive that.
let disposeIn = 0
const DISPOSE_FRAMES = 2
let avatarEntity: Entity | null = null
let cameraEntity: Entity | null = null
let podiumEntities: Entity[] = []
// The Shop's stage image on screen (previewStage), in place of the backdrop bands and the podium.
let shopStage: Rect | null = null
// Bumped for every emote the Shop plays: the engine plays a trigger whose timestamp is new.
let emoteTrigger = 0
let lastShapeKey = ''
// A new look just took the stage: the avatar cheers once it faces the camera.
let cheerPending = false
// The lobby stage swaps between two avatars, one on show and one standing by, so that a new look
// loads out of sight and the one on show never goes missing: a look wanted on the stage goes on
// the standby first, and the stage swaps once the engine reports it loaded. The next RANDOMIZE's
// look is loaded on the standby ahead, once the one on show has loaded, so its swap is instant.
let standbyEntity: Entity | null = null
let shownKey = ''
let standbyKey = ''
// the curated default look numbers behind the keys, for the page (`lookShown`); the requested
// one is the page's pick, held until the stage takes it up (a shape change without one is an equip)
let shownLook: number | null = null
let standbyLook: number | null = null
let requestedLook: number | null = null
// the turn under way, and where the avatar on show is facing (YAW_FRONT when still)
let turn: { entity: Entity; from: number; to: number; t: number } | null = null
let shownYaw = YAW_FRONT
// the standby has the wanted look loaded: it takes the stage once the shown one has turned away
let swapReady = false
// frames left to wait for the look a RANDOMIZE promised, turned away for it in advance
let expecting = 0
let announce: ((look: number) => void) | null = null
// whether the standby's look was asked for (a RANDOMIZE), as opposed to loaded ahead
let standbyWanted = false
let wanted: { key: string; shape: ShapeData; look: number | null } | null = null
let preload: { key: string; shape: ShapeData; look: number } | null = null
let showLoad: LoadWatch | null = null
let standbyLoad: LoadWatch | null = null
let shownLoaded = false

/** An avatar's load on its entity, from the engine's reports since it was given its look. */
type LoadWatch = {
  entity: Entity
  since: number
  frames: number
  done: boolean
}

function latestLoad(entity: Entity): { timestamp: number; state: number } | null {
  if (!AssetLoadLoadingState.has(entity)) return null
  let latest: { timestamp: number; state: number } | null = null
  for (const report of AssetLoadLoadingState.get(entity)) {
    if (latest == null || report.timestamp > latest.timestamp) latest = { timestamp: report.timestamp, state: report.currentState }
  }
  return latest
}

function watchLoad(entity: Entity): LoadWatch {
  return { entity, since: latestLoad(entity)?.timestamp ?? 0, frames: 0, done: false }
}

// Done once the engine reports the load finished (well or not), or after long enough that it
// never will.
function pollLoad(watch: LoadWatch): void {
  if (watch.done) return
  const latest = latestLoad(watch.entity)
  const ended = latest != null && latest.timestamp > watch.since && [LOADING_FINISHED, LOADING_FINISHED_WITH_ERROR, LOADING_NOT_FOUND].includes(latest.state)
  if (ended || ++watch.frames >= LOBBY_LOOK_FRAMES) watch.done = true
}
let framing = FRAMING.body
let framingFrom = FRAMING.body
let framingTo = FRAMING.body
let framingT = 1
// The page's zoom over the framing in use (previewZoom), and how far the view's centre has moved up (metres) to
// keep the height the page zoomed toward in place: 1 and 0 are the framing as it is.
let zoom = 1
let zoomShift = 0
// Non-persisting preview override (selecting an item in the Backpack): when set, the preview
// avatar wears these urns instead of the player's actual equipped set. null = no override.
let previewUrns: string[] | null = null
let frameWaiters: Array<{ frames: number; done: () => void }> = []

// Render-target bounds, physical px. Fit inside the engine's 2048 cap here — a one-axis
// engine-side clamp would break the aspect.
const MIN_RES = 256
const MAX_RES = 2048

// The page reports it with each rect; 1 is the safe read (CSS px == physical px).
let dpr = 1

// TextureCamera resolution: the cutout's physical pixels (CSS px x dpr) at its exact aspect,
// so a 1:1 stretch never distorts and the render target follows the window.
function camRes(r: Rect): { width: number; height: number } {
  const aspect = r.height > 0 ? r.width / r.height : 0.55
  let h = Math.min(MAX_RES, Math.max(MIN_RES, Math.round(r.height * dpr)))
  let w = Math.max(64, Math.round(h * aspect))
  if (w > MAX_RES) {
    // ultra-wide cutout: fit the other way, keep the aspect
    w = MAX_RES
    h = Math.max(MIN_RES, Math.round(w / aspect))
  }
  return { width: w, height: h }
}

type ShapeData = {
  bodyShape?: string
  eyeColor?: { r: number; g: number; b: number }
  hairColor?: { r: number; g: number; b: number }
  skinColor?: { r: number; g: number; b: number }
  wearables: string[]
  emotes: string[]
  forceRender: string[]
}

// `override`: the Backpack's preview (item selected, not equipped) takes precedence over the
// look's set, so selecting shows the item without adding it to the look.
function shapeOf(look: AvatarLook, override: string[] | null = null): ShapeData {
  return {
    bodyShape: look.bodyShape || undefined,
    eyeColor: look.eyes ?? undefined,
    hairColor: look.hair ?? undefined,
    skinColor: look.skin ?? undefined,
    wearables: override ?? look.wearables,
    emotes: [],
    forceRender: look.forceRender
  }
}

const shapeKey = (shape: ShapeData): string => JSON.stringify(shape)

function setShape(entity: Entity, shape: ShapeData): void {
  const mut = AvatarShape.getMutableOrNull(entity)
  if (mut != null) Object.assign(mut, shape)
  else AvatarShape.create(entity, { ...shape, id: getPlayer()?.userId ?? '', name: undefined, talking: false })
}

function createPreview(): void {
  if (avatarEntity != null || rect == null) return
  const res = camRes(rect)
  const a = engine.addEntity()
  const c = engine.addEntity()

  const lobby = stage === 'lobby'
  // the shape is added by syncShape once the player's look is known: an empty one would render
  // the default body, which the engine keeps on show until the real look has loaded
  CameraLayers.create(a, { layers: [LAYER] })
  Transform.create(a, {
    position: Vector3.create(8, 0, 8),
    rotation: FACING,
    scale: lobby ? Vector3.One() : Vector3.create(2, 2, 2)
  })
  if (lobby) {
    const standby = engine.addEntity()
    CameraLayers.create(standby, { layers: [STANDBY_LAYER] })
    Transform.create(standby, { position: Vector3.create(8, 0, 8), rotation: FACING, scale: Vector3.One() })
    standbyEntity = standby
  }
  // Podium under the avatar (preview layer only), the parity backpack's platform; the lobby's
  // podium is the engine stage's.
  const podium = lobby ? [] : PODIUM_LAYERS.map((layer) => {
    const e = engine.addEntity()
    MeshRenderer.setCylinder(e, 1, 1)
    Material.setPbrMaterial(e, { albedoColor: layer.color, metallic: 0, roughness: 0.6 })
    CameraLayers.create(e, { layers: [LAYER] })
    Transform.create(e, { position: Vector3.create(8, layer.y, 8), scale: Vector3.create(layer.scale, layer.height, layer.scale) })
    return e
  })

  CameraLayer.create(c, {
    layer: LAYER,
    // the Backpack is lit flat; the lobby's engine stage brings its own key light
    directionalLight: false,
    showAvatars: false,
    showSkybox: false,
    showFog: false,
    ambientBrightnessOverride: lobby ? 2 : 5
  })
  TextureCamera.create(c, {
    width: res.width,
    height: res.height,
    layer: LAYER,
    clearColor: Color4.create(0, 0, 0, 0),
    mode: lobby
      ? { $case: 'perspective', perspective: { fieldOfView: LOBBY_FOV } }
      : { $case: 'orthographic', orthographic: { verticalRange: framing.range } },
    volume: 1
  })
  Transform.create(
    c,
    lobby
      ? {
          position: Vector3.create(8, LOBBY_TARGET_Y + LOBBY_CAMERA_RISE, 8 - LOBBY_CAMERA_DISTANCE),
          rotation: Quaternion.fromEulerDegrees((Math.atan2(LOBBY_CAMERA_RISE, LOBBY_CAMERA_DISTANCE) * 180) / Math.PI, 0, 0)
        }
      : {
          position: Vector3.create(8, framing.centerY + PITCH_DROP, 8 - CAMERA_DISTANCE),
          rotation: Quaternion.fromEulerDegrees(CAMERA_PITCH, 0, 0)
        }
  )

  avatarEntity = a
  cameraEntity = c
  podiumEntities = podium
  if (lobby) showStage()
  if (shopStage != null) showPodium(false)
  lastShapeKey = ''
  shownKey = ''
  standbyKey = ''
  shownLook = null
  standbyLook = null
  turn = null
  shownYaw = YAW_FRONT
  swapReady = false
  expecting = 0
  wanted = null
  standbyWanted = false
  showLoad = null
  standbyLoad = null
  shownLoaded = false
  syncShape()
}

// Bumped whenever a stage is built, so an announcement for a stage since rebuilt never fires.
let stageGeneration = 0

async function announceLobbyStage(ctx: Ctx): Promise<void> {
  const generation = ++stageGeneration
  const current = (): boolean => generation === stageGeneration && stage === 'lobby' && avatarEntity != null
  // the avatar renders nothing until its look has arrived and loaded; a reroll meanwhile keeps
  // the stand-in until the new look is on show and facing front
  const landed = (): boolean => shownLoaded && wanted == null && turn == null
  // a stage already on show (a page taking it over) has nothing left to settle
  const settled = landed()
  for (let waited = 0; current() && !landed() && waited < 2 * LOBBY_LOOK_FRAMES; waited += 5) await waitFrames(5)
  if (!settled) await waitFrames(LOBBY_SETTLE_FRAMES)
  if (current()) ctx.send({ kind: 'lobbyStageReady' })
}

// Ask the engine for the lobby's 3D stage around the avatar, in the look the page named.
function showStage(): void {
  stageUp = true
  greeted = false
  flavourIn = nextFlavour()
  BevyApi.setLobbyStage?.({ preset: stagePreset, layer: LAYER, feet: LOBBY_FEET }).catch((e) => {
    console.error('[avatarPreview] lobby stage', e)
  })
}

function hideStage(): void {
  if (!stageUp) return
  stageUp = false
  BevyApi.setLobbyStage?.(null).catch((e) => {
    console.error('[avatarPreview] lobby stage', e)
  })
}

function nextFlavour(): number {
  return FLAVOUR_SECONDS[0] + Math.random() * (FLAVOUR_SECONDS[1] - FLAVOUR_SECONDS[0])
}

// Play an emote on the avatar on show (a new timestamp replays the same one).
function playEmote(urn: string): void {
  if (avatarEntity == null) return
  const shape = AvatarShape.getMutableOrNull(avatarEntity)
  if (shape == null) return
  shape.expressionTriggerId = urn
  shape.expressionTriggerTimestamp = ++emoteTrigger
}

// The lobby avatar's idle life, while its look is on show and facing the camera.
function pollEmotes(dt: number): void {
  if (!idleEmotes || !shownLoaded || turn != null) return
  if (!greeted) {
    greeted = true
    playEmote(INTRO_EMOTE)
    flavourIn = nextFlavour()
    return
  }
  flavourIn -= dt
  if (flavourIn > 0) return
  flavourIn = nextFlavour()
  playEmote(FLAVOUR_EMOTES[Math.floor(Math.random() * FLAVOUR_EMOTES.length)])
}

function disposePreview(): void {
  hideStage()
  if (avatarEntity != null) engine.removeEntity(avatarEntity)
  if (standbyEntity != null) engine.removeEntity(standbyEntity)
  if (cameraEntity != null) engine.removeEntity(cameraEntity)
  podiumEntities.forEach((e) => engine.removeEntity(e))
  avatarEntity = null
  standbyEntity = null
  cameraEntity = null
  podiumEntities = []
  shownKey = ''
  standbyKey = ''
  shownLook = null
  standbyLook = null
  turn = null
  shownYaw = YAW_FRONT
  swapReady = false
  expecting = 0
  wanted = null
  standbyWanted = false
  preload = null
  showLoad = null
  standbyLoad = null
  shownLoaded = false
  zoom = 1
  zoomShift = 0
}

/** Re-read the look into the shape when it changes (a pick from the page, an equip via React →
 *  setAvatar). The Backpack's preview changes in place; the lobby stage loads the new look on its
 *  standby and swaps to it once loaded. */
export function syncShape(look?: number): void {
  if (look != null) requestedLook = look
  if (avatarEntity == null) return
  const current = currentLook()
  if (!current?.bodyShape) return
  const pick = requestedLook
  requestedLook = null
  const shape = shapeOf(current, previewUrns)
  const key = shapeKey(shape)
  if (key === lastShapeKey) return
  lastShapeKey = key
  if (key === shownKey && stage === 'lobby' && AvatarShape.has(avatarEntity)) {
    // the look on show again (a RANDOMIZE taken back before its swap): it turns back to the front
    wanted = null
    swapReady = false
    standbyWanted = false
    expecting = 0
    if (shownYaw !== YAW_FRONT) startTurn(avatarEntity, shownYaw, YAW_FRONT)
    return
  }
  if (stage !== 'lobby' || standbyEntity == null || !AvatarShape.has(avatarEntity)) {
    setShape(avatarEntity, shape)
    if (stage === 'lobby') {
      shownKey = key
      shownLook = pick
      shownLoaded = false
      showLoad = watchLoad(avatarEntity)
    }
    return
  }
  wanted = { key, shape, look: pick }
  expecting = 0
  // the avatar on show turns away meanwhile; the new one takes the stage once it has, and is loaded
  if (shownYaw !== YAW_AWAY) startTurn(avatarEntity, shownYaw, YAW_AWAY)
  // already loaded on the standby (a look loaded ahead, or the one before): on show now if the
  // shown avatar has already turned away (RANDOMIZE turns it ahead of the look), else once it has
  if (key === standbyKey && standbyLoad == null) {
    if (shownYaw === YAW_AWAY && turn == null) swapToStandby()
    else swapReady = true
    return
  }
  loadOnStandby()
}

/** RANDOMIZE was hit: the avatar on show turns away now, ahead of the look (which the page is still
 *  fetching); it turns back if none comes. */
export function expectLook(): void {
  if (stage !== 'lobby' || avatarEntity == null || !AvatarShape.has(avatarEntity) || !shownLoaded) return
  expecting = LOBBY_LOOK_FRAMES
  if (shownYaw !== YAW_AWAY && turn?.to !== YAW_AWAY) startTurn(avatarEntity, shownYaw, YAW_AWAY)
}

function startTurn(entity: Entity, from: number, to: number): void {
  turn = { entity, from, to, t: 0 }
}

// Advance the turn under way; done, a look that was waiting for the shown avatar to face away
// can take the stage, and a new one facing front is announced to the page.
function pollTurn(dt: number): void {
  if (turn == null) return
  turn.t = Math.min(1, turn.t + dt / TURN_SECONDS)
  const e = turn.t * turn.t * (3 - 2 * turn.t)
  const yaw = turn.from + (turn.to - turn.from) * e
  if (Transform.has(turn.entity)) Transform.getMutable(turn.entity).rotation = Quaternion.fromEulerDegrees(0, yaw, 0)
  if (turn.entity === avatarEntity) shownYaw = yaw
  if (turn.t < 1) return
  const done = turn
  turn = null
  if (done.entity !== avatarEntity) return
  if (done.to === YAW_FRONT && shownLook != null && announce != null) announce(shownLook)
  if (done.to === YAW_FRONT && cheerPending) {
    cheerPending = false
    if (idleEmotes && greeted) {
      playEmote(LOOK_CHANGED_EMOTE)
      flavourIn = nextFlavour()
    }
  }
  if (done.to === YAW_AWAY && swapReady) swapToStandby()
  // a look loaded meanwhile (one asked for after the one just shown): turn away for it
  else if (done.to === YAW_FRONT && swapReady) startTurn(done.entity, YAW_FRONT, YAW_AWAY)
}

// The look wanted on show goes on the standby (unless already there, loaded ahead); the next
// RANDOMIZE's look goes there once the one on show has loaded and nothing else is wanted, nor on
// its way (a RANDOMIZE's look, usually the one loaded ahead, must find it there).
function loadOnStandby(): void {
  if (standbyEntity == null || standbyLoad != null) return
  const next = wanted != null && wanted.key !== standbyKey ? wanted : wanted == null && expecting === 0 && preload != null && shownLoaded && preload.key !== standbyKey ? preload : null
  if (next == null) return
  setShape(standbyEntity, next.shape)
  standbyKey = next.key
  standbyLook = next.look
  standbyWanted = next === wanted
  standbyLoad = watchLoad(standbyEntity)
}

// The standby, loaded with the look wanted on show, takes the stage; the avatar that was on show
// becomes the standby, still wearing its look (a look dropped and replaced in one frame never
// reports loaded; and a RANDOMIZE back to it then costs nothing).
function swapToStandby(): void {
  if (avatarEntity == null || standbyEntity == null) return
  const shown = avatarEntity
  const standby = standbyEntity
  CameraLayers.getMutable(standby).layers = [LAYER]
  CameraLayers.getMutable(shown).layers = [STANDBY_LAYER]
  Transform.getMutable(standby).rotation = Quaternion.fromEulerDegrees(0, YAW_BACK, 0)
  avatarEntity = standby
  standbyEntity = shown
  const wasShown = shownKey
  shownKey = standbyKey
  standbyKey = wasShown
  const wasShownLook = shownLook
  shownLook = standbyLook
  standbyLook = wasShownLook
  standbyWanted = false
  swapReady = false
  if (wanted?.key === shownKey) wanted = null
  shownLoaded = true
  shownYaw = YAW_BACK
  startTurn(standby, YAW_BACK, YAW_FRONT)
  cheerPending = true
}

function pollStage(dt: number): void {
  pollTurn(dt)
  if (expecting > 0 && --expecting === 0 && wanted == null && avatarEntity != null && shownYaw !== YAW_FRONT) {
    startTurn(avatarEntity, shownYaw, YAW_FRONT)
  }
  if (showLoad != null) {
    pollLoad(showLoad)
    if (showLoad.done) {
      showLoad = null
      shownLoaded = true
      if (shownLook != null && announce != null) announce(shownLook)
    }
  }
  if (standbyLoad != null) {
    pollLoad(standbyLoad)
    if (standbyLoad.done) {
      standbyLoad = null
      // a loaded look that was asked for shows, even if another was asked for since: that one
      // then loads on what was on show. It waits for the shown avatar to have turned away.
      if (wanted?.key === standbyKey || standbyWanted) {
        if (shownYaw === YAW_AWAY && turn == null) swapToStandby()
        else {
          swapReady = true
          // facing front and still (the look before took the stage while this one was wanted)
          if (turn == null && avatarEntity != null) startTurn(avatarEntity, shownYaw, YAW_AWAY)
        }
      }
    }
  }
  loadOnStandby()
}

export function registerAvatarPreview(ctx: Ctx): void {
  ctx.on('engineViewport', (msg) => {
    if (msg.region !== 'avatarPreview' && msg.region !== 'lobby') return
    const nextStage: Stage = msg.region === 'lobby' ? 'lobby' : 'backpack'
    // a region closing after another took over (lobby → Backpack) must not dispose the new one
    if (msg.rect == null && nextStage !== stage) return
    if (nextStage !== stage) {
      disposePreview()
      stage = nextStage
      framing = framingFrom = framingTo = FRAMING.body
      framingT = 1
    }
    if (msg.rect == null) {
      if (avatarEntity != null) disposeIn = DISPOSE_FRAMES
      else rect = null
      return
    }
    disposeIn = 0
    if (nextStage === 'lobby') {
      idleEmotes = msg.idleEmotes === true
      const preset = msg.stagePreset ?? stagePreset
      if (preset !== stagePreset) {
        stagePreset = preset
        if (stageUp) showStage()
      }
    }
    // the same region again (a page re-reporting its viewport) leaves the stage as it is
    const same = rect != null && rect.x === msg.rect.x && rect.y === msg.rect.y && rect.width === msg.rect.width && rect.height === msg.rect.height && dpr === (msg.dpr ?? 1)
    rect = msg.rect
    dpr = msg.dpr ?? 1
    if (avatarEntity == null) createPreview()
    else if (!same && cameraEntity != null) {
      // Window resized → re-size the render target to the hole, not just re-aspect it.
      const res = camRes(rect)
      const cam = TextureCamera.getMutableOrNull(cameraEntity)
      if (cam != null && (cam.width !== res.width || cam.height !== res.height)) {
        cam.width = res.width
        cam.height = res.height
      }
    }
    // told again for a stage carried over (the guest start page's into the lobby, the lobby's
    // back to it on logout): the page taking it over may have put its stand-in back up
    if (stage === 'lobby') {
      announceLobbyStage(ctx).catch((e) => {
        console.error('[avatarPreview] lobby stage', e)
      })
    }
  })

  // The Backpack's selected category: ease the camera to frame that part of the avatar.
  ctx.on('previewFocus', (msg) => {
    if (stage !== 'backpack' || shopStage != null) return
    const to = FRAMING[msg.focus] ?? FRAMING.body
    if (to === framingTo) return
    framingFrom = framing
    framingTo = to
    framingT = 0
  })
  ctx.push(() => {
    if (disposeIn > 0 && --disposeIn === 0) {
      disposePreview()
      rect = null
    }
    const due = frameWaiters.filter((w) => --w.frames <= 0)
    frameWaiters = frameWaiters.filter((w) => w.frames > 0)
    due.forEach((w) => {
      w.done()
    })
  })
  announce = (look) => {
    ctx.send({ kind: 'lookShown', look })
  }
  ctx.push((dt) => {
    if (stage === 'lobby' && avatarEntity != null) {
      pollStage(dt)
      pollEmotes(dt)
    }
  })
  ctx.push((dt) => {
    if (framingT >= 1 || cameraEntity == null || stage !== 'backpack') return
    framingT = Math.min(1, framingT + dt / FOCUS_SECONDS)
    const e = framingT * framingT * (3 - 2 * framingT)
    framing = {
      range: framingFrom.range + (framingTo.range - framingFrom.range) * e,
      centerY: framingFrom.centerY + (framingTo.centerY - framingFrom.centerY) * e
    }
    applyFraming(framing)
  })

  // The look the next RANDOMIZE shows, loaded on the lobby's standby ahead of it.
  ctx.on('preloadLook', (msg) => {
    const shape = shapeOf(msg.avatar)
    preload = { key: shapeKey(shape), shape, look: msg.look }
    if (stage === 'lobby') loadOnStandby()
  })

  // The Shop draws its stage around the avatar's window: the same image goes behind the avatar, standing on
  // the floor the image has, so the Backpack's podium steps aside.
  ctx.on('previewStage', (msg) => {
    shopStage = msg.rect
    showPodium(shopStage == null)
    framing = framingFrom = framingTo = shopStage != null ? SHOP_FRAMING : FRAMING.body
    framingT = 1
    applyFraming(framing)
  })

  // The page's drag over the preview turns the avatar, and its wheel zooms the camera: the page drives both,
  // as the drag-lock (onMouseDragLocked below) doesn't reach the preview in every host. Both are the preview's
  // (the Backpack's, the Shop's) alone: a late one never turns or zooms the lobby's avatar.
  ctx.on('previewRotate', (msg) => {
    if (stage === 'lobby') turnLobbyAvatar(msg.degrees)
    else turnAvatar(msg.degrees)
  })
  ctx.on('previewZoom', (msg) => {
    if (stage !== 'backpack') return
    const next = Math.min(PREVIEW_MAX_ZOOM, Math.max(1, Number.isFinite(msg.zoom) ? msg.zoom : 1))
    const anchor = Number.isFinite(msg.anchor) ? Math.min(1, Math.max(0, msg.anchor)) : 0.5
    // The height under the anchor stays where it is: the same share of the view above the centre, before and after.
    const above = 0.5 - anchor
    const height = framing.centerY + shiftWithin(framing, zoom, zoomShift) + (framing.range / zoom) * above
    zoomShift = shiftWithin(framing, next, height - (framing.range / next) * above - framing.centerY)
    zoom = next
    applyFraming(framing)
  })

  // The Shop's emote try-on: played once on the preview avatar, again for every ask; null stops it (an
  // empty trigger on a scene's avatar is the engine's stop).
  ctx.on('previewEmote', (msg) => {
    playEmote(msg.urn ?? '')
  })

  // Selecting an item in the Backpack previews it on the avatar without persisting (null reverts).
  ctx.on('previewAvatar', (msg) => {
    previewUrns = msg.urns
    lastShapeKey = '' // force the next syncShape to apply the new (or reverted) set
    syncShape()
  })

  // Keep the preview in sync with the player. Poll fast until the avatar has a shape (the
  // player's look may not be known the instant the stage opens), then throttle to ~2/s to
  // pick up equips.
  let acc = 0
  ctx.push((dt) => {
    if (rect == null || avatarEntity == null) return
    acc += dt
    const ready = lastShapeKey !== ''
    if (acc < (ready ? 0.5 : 0.1)) return
    acc = 0
    syncShape()
  })
}

export async function waitFrames(frames: number): Promise<void> {
  await new Promise<void>((resolve) => {
    frameWaiters.push({ frames, done: resolve })
  })
}

function showPodium(visible: boolean): void {
  if (stage !== 'backpack') return
  podiumEntities.forEach((e, i) => {
    const layer = PODIUM_LAYERS[i]
    Transform.getMutable(e).scale = visible ? Vector3.create(layer.scale, layer.height, layer.scale) : Vector3.Zero()
  })
}

// A zoomed view's centre moves at most as far as keeps it inside the framing it zooms (none at zoom 1).
function shiftWithin(f: { range: number }, z: number, shift: number): number {
  const room = (f.range - f.range / z) / 2
  return Math.max(-room, Math.min(room, shift))
}

// The framing on the camera, zoomed `z` times with its centre moved `shift` up (the page's zoom unless told
// otherwise), the view kept inside the framing.
function applyFraming(f: { range: number; centerY: number }, z: number = zoom, shift: number = zoomShift): void {
  if (cameraEntity == null || stage !== 'backpack') return
  const cam = TextureCamera.getMutableOrNull(cameraEntity)
  if (cam?.mode?.$case === 'orthographic') cam.mode.orthographic.verticalRange = f.range / z
  Transform.getMutable(cameraEntity).position = Vector3.create(8, f.centerY + shiftWithin(f, z, shift) + PITCH_DROP, 8 - CAMERA_DISTANCE)
}

/** The preview avatar, whole body, facing forward, without the podium, as a PNG data URL — or null
 *  when the preview isn't open or the engine can't capture. */
export async function captureAvatarThumbnail(): Promise<string | null> {
  if (avatarEntity == null || cameraEntity == null || BevyApi.consoleCommand == null) return null
  const avatar = avatarEntity
  const rotation = Transform.get(avatar).rotation
  Transform.getMutable(avatar).rotation = FACING
  framingT = 1
  // The whole body, whatever the page has zoomed to.
  applyFraming(FRAMING.body, 1, 0)
  const podium = podiumEntities
  podium.forEach((e) => { Transform.getMutable(e).scale = Vector3.Zero() })
  try {
    await waitFrames(3)
    const png = await BevyApi.consoleCommand('texture_camera_screenshot', [String(LAYER), String(THUMBNAIL_SIZE)])
    return `data:image/png;base64,${png}`
  } catch (e) {
    console.error('[avatarPreview] thumbnail capture failed', e)
    return null
  } finally {
    if (avatarEntity === avatar) {
      Transform.getMutable(avatar).rotation = rotation
      if (framingT >= 1) framing = framingTo
      applyFraming(framing)
      showPodium(shopStage == null)
    }
  }
}

function rotateAvatar(): void {
  const pointer = PrimaryPointerInfo.getOrNull(engine.RootEntity)
  const deltaX = pointer?.screenDelta?.x ?? 0
  if (deltaX !== 0) turnAvatar(deltaX * ROTATION_FACTOR)
}

// The lobby avatar turns by the page's drag, unless a look is changing; its yaw is the one the
// look changes turn from, so a drag is never undone.
function turnLobbyAvatar(degrees: number): void {
  if (avatarEntity == null || turn != null || !Number.isFinite(degrees) || degrees === 0) return
  shownYaw = YAW_FRONT + ((((shownYaw - YAW_FRONT + degrees) % 360) + 540) % 360) - 180
  Transform.getMutable(avatarEntity).rotation = Quaternion.fromEulerDegrees(0, shownYaw, 0)
}

// Turns the preview avatar `degrees` about its vertical axis.
function turnAvatar(degrees: number): void {
  if (avatarEntity == null || !Number.isFinite(degrees) || degrees === 0) return
  const qY = Quaternion.fromAngleAxis(degrees, Vector3.create(0, 1, 0))
  const cur = Transform.get(avatarEntity).rotation
  Transform.getMutable(avatarEntity).rotation = Quaternion.multiply(
    Quaternion.create(cur.x, cur.y, cur.z, cur.w),
    qY
  )
}

function renderLobbyStage(r: Rect, camera: Entity): ReactEcs.JSX.Element {
  // the engine's stage is opaque over the whole frame, so the camera's image is the lobby's
  return (
    <UiEntity
      uiTransform={{ positionType: 'absolute', position: { left: r.x, top: r.y }, width: r.width, height: r.height, overflow: 'hidden' }}
      uiBackground={{ videoTexture: { videoPlayerEntity: camera }, textureMode: 'stretch' }}
    />
  )
}

export function renderAvatarPreview(): ReactEcs.JSX.Element | null {
  if (rect == null || cameraEntity == null) return null
  const r = rect
  if (stage === 'lobby') return renderLobbyStage(r, cameraEntity)
  return (
    // Full-screen opaque base: the live world can never show through React's transparent
    // avatar cutout, even if the reported rect and the cutout don't line up to the pixel.
    // React paints everything except the avatar column, so this purple only ever shows
    // inside that column — it's a guaranteed backdrop, not a visible full-screen fill.
    <UiEntity
      uiTransform={{ positionType: 'absolute', position: { left: 0, top: 0 }, width: '100%', height: '100%' }}
      uiBackground={{ color: BACKDROP_BOTTOM }}
    >
      {/* Framed two-tone backdrop + avatar, positioned at the React cutout rect. */}
      <UiEntity
        uiTransform={{ positionType: 'absolute', position: { left: r.x, top: r.y }, width: r.width, height: r.height, overflow: 'hidden' }}
        uiBackground={{ color: BACKDROP_BOTTOM }}
      >
        {shopStage != null ? (
          // The Shop's stage, placed as the page places it: its window into it shows the same pixels.
          <UiEntity
            uiTransform={{ positionType: 'absolute', position: { left: shopStage.x - r.x, top: shopStage.y - r.y }, width: shopStage.width, height: shopStage.height }}
            uiBackground={{ texture: { src: SHOP_STAGE }, textureMode: 'stretch' }}
          />
        ) : (
          // The page's left-column purple, as thin bands so it reads as a smooth gradient.
          BACKDROP_BANDS.map((color, i) => (
            <UiEntity
              key={i}
              uiTransform={{ positionType: 'absolute', position: { top: `${(i * 100) / BACKDROP_BANDS.length}%` }, width: '100%', height: `${100 / BACKDROP_BANDS.length + 0.5}%` }}
              uiBackground={{ color }}
            />
          ))
        )}
        {/* Avatar — camera resolution matches the rect aspect, so a 1:1 fill never distorts.
            Drag over it to rotate (the engine drag-lock passes through React's transparent cutout). */}
        <UiEntity
          uiTransform={{ positionType: 'absolute', width: '100%', height: '100%' }}
          uiBackground={{ videoTexture: { videoPlayerEntity: cameraEntity }, textureMode: 'stretch' }}
          onMouseDragLocked={rotateAvatar}
        />
      </UiEntity>
    </UiEntity>
  )
}
