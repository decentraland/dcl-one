// The passport's avatar: any player's look, drawn live in the passport's left column.
//
// The page leaves the column see-through and sends its screen rect with the look (passportPreview). The
// avatar stands on a camera layer of its own, with a perspective TextureCamera, and is drawn into a UI node at that rect, behind the React DOM, over the part of
// the passport panel's background that lies under the column (images/passport-*.png, drawn by
// ../../scripts/passport-backgrounds.mjs from the panel's own CSS, with the glow and ground shadow).
//
// It is a stage of its own, apart from ./avatarPreview's: it draws over whichever of those is up (the
// Backpack's, the Shop's, the lobby's), which stops drawing meanwhile. The page hears when the avatar is
// loaded and drawn, or cannot be (passportPreviewState), and shows its picture of the avatar until then.
import ReactEcs, { UiEntity } from '@dcl/react-ecs'
import { AssetLoadLoadingState, AvatarShape, CameraLayer, CameraLayers, TextureCamera, Transform, engine } from '@dcl/sdk/ecs'
import { Color4, Quaternion, Vector3 } from '@dcl/sdk/math'
import type { Entity } from '@dcl/ecs'
import type { Ctx } from '../bridge'
import type { PassportBackground, PassportPreviewRequest } from '@explorer-core/engine/protocol'
import { holdPreview, releasePreview } from './avatarPreview'
import { dragTurn, restingTurn, stepTurn } from './passportTurn'

type Open = NonNullable<PassportPreviewRequest['open']>

// A camera layer no other stage of this scene uses (./avatarPreview has 10 to 13; a scene has 15).
const LAYER = 14
// Away from the other stages' avatars, which stand at (8, 0, 8) on their own layers.
const FEET = Vector3.create(8, 0, 24)
const YAW_FRONT = 180

// The column and the avatar's picture in it, in pixels of the 1920x1080 canvas: the column is 514x928, and
// the picture, 960 tall, is centred 498 below its top.
// Only the column's width of it is ever seen, so only that much is drawn.
const COLUMN_HEIGHT = 928
const PICTURE_HEIGHT = 960
const PICTURE_TOP = 498 - PICTURE_HEIGHT / 2

// The camera sees 24 degrees up and down, and looks at a point above the avatar's feet from further up and
// back, with that point this far down the picture.
const FOV = (24 * Math.PI) / 180
const TARGET_Y = 0.8
const CAMERA_RISE = 1
const CAMERA_DISTANCE = 6
const TARGET_SCREEN_Y = 0.55
const CAMERA_PITCH = Math.atan2(CAMERA_RISE, CAMERA_DISTANCE) - Math.atan((2 * TARGET_SCREEN_Y - 1) * Math.tan(FOV / 2))
const FAR = 20

const MAX_RES = 2048
const BACKGROUNDS: Record<PassportBackground, string> = {
  overview: 'images/passport-overview.png',
  photos: 'images/passport-photos.png',
  creations: 'images/passport-creations.png'
}
const INTRO_EMOTE = 'wave'

// decentraland.sdk.components.common.LoadingState (a const enum the SDK does not export at runtime)
const LOADING_NOT_FOUND = 2
const LOADING_FINISHED_WITH_ERROR = 3
const LOADING_FINISHED = 4
// Seconds to wait for an avatar's load at all, and frames for a loaded avatar's parts to be drawn in (see
// ./avatarPreview's LOOK_DRAWN_FRAMES).
const LOAD_SECONDS = 10
const DRAWN_FRAMES = 45

let open: Open | null = null
let avatarEntity: Entity | null = null
let cameraEntity: Entity | null = null
let lookKey = ''
let yaw = YAW_FRONT
let turn = restingTurn()
let emoteTrigger = 0
// The avatar's load: the newest report before the look was given, the seconds waited since, and, once the
// engine says it loaded, the frames left for its parts to be drawn.
let load: { since: number; seconds: number; drawnIn: number | null } | null = null

function latestLoad(entity: Entity): { timestamp: number; state: number } | null {
  if (!AssetLoadLoadingState.has(entity)) return null
  let latest: { timestamp: number; state: number } | null = null
  for (const report of AssetLoadLoadingState.get(entity)) {
    if (latest == null || report.timestamp > latest.timestamp) latest = { timestamp: report.timestamp, state: report.currentState }
  }
  return latest
}

function resolution(o: Open): { width: number; height: number } {
  const scale = (o.rect.height / COLUMN_HEIGHT) * o.dpr
  const height = Math.min(MAX_RES, Math.max(64, Math.round(PICTURE_HEIGHT * scale)))
  const width = Math.max(64, Math.round((height * o.rect.width) / ((PICTURE_HEIGHT * o.rect.height) / COLUMN_HEIGHT)))
  return { width: Math.min(MAX_RES, width), height }
}

function shapeOf(o: Open) {
  return {
    bodyShape: o.look.bodyShape || undefined,
    eyeColor: o.look.eyes ?? undefined,
    hairColor: o.look.hair ?? undefined,
    skinColor: o.look.skin ?? undefined,
    wearables: o.look.wearables,
    emotes: [],
    forceRender: o.look.forceRender
  }
}

function build(o: Open): void {
  const avatar = engine.addEntity()
  const camera = engine.addEntity()
  CameraLayers.create(avatar, { layers: [LAYER] })
  Transform.create(avatar, { position: FEET, rotation: Quaternion.fromEulerDegrees(0, YAW_FRONT, 0) })
  CameraLayer.create(camera, {
    layer: LAYER,
    // lit flat, as the Backpack's avatar is
    directionalLight: false,
    showAvatars: false,
    showSkybox: false,
    showFog: false,
    ambientBrightnessOverride: 5
  })
  TextureCamera.create(camera, {
    ...resolution(o),
    layer: LAYER,
    clearColor: Color4.create(0, 0, 0, 0),
    farPlane: FAR,
    mode: { $case: 'perspective', perspective: { fieldOfView: FOV } },
    volume: 0
  })
  Transform.create(camera, {
    position: Vector3.create(FEET.x, FEET.y + TARGET_Y + CAMERA_RISE, FEET.z - CAMERA_DISTANCE),
    rotation: Quaternion.fromEulerDegrees((CAMERA_PITCH * 180) / Math.PI, 0, 0)
  })
  avatarEntity = avatar
  cameraEntity = camera
  yaw = YAW_FRONT
  turn = restingTurn()
  lookKey = ''
}

function dispose(): void {
  if (avatarEntity != null) engine.removeEntity(avatarEntity)
  if (cameraEntity != null) engine.removeEntity(cameraEntity)
  avatarEntity = null
  cameraEntity = null
  load = null
  lookKey = ''
}

function wear(o: Open): void {
  if (avatarEntity == null) return
  const shape = shapeOf(o)
  const key = JSON.stringify(shape)
  if (key === lookKey) return
  const first = lookKey === ''
  lookKey = key
  const mut = AvatarShape.getMutableOrNull(avatarEntity)
  if (mut != null) Object.assign(mut, shape)
  else AvatarShape.create(avatarEntity, { ...shape, id: '', name: undefined, talking: false })
  // the page was told about the first look only: a later one changes in place
  if (first) load = { since: latestLoad(avatarEntity)?.timestamp ?? 0, seconds: 0, drawnIn: null }
}

function fit(o: Open): void {
  const camera = cameraEntity != null ? TextureCamera.getMutableOrNull(cameraEntity) : null
  if (camera == null) return
  const res = resolution(o)
  if (camera.width !== res.width || camera.height !== res.height) {
    camera.width = res.width
    camera.height = res.height
  }
}

// A new timestamp replays the same emote.
function playEmote(urn: string): void {
  const shape = avatarEntity != null ? AvatarShape.getMutableOrNull(avatarEntity) : null
  if (shape == null) return
  shape.expressionTriggerId = urn
  shape.expressionTriggerTimestamp = ++emoteTrigger
}

function turnBy(degrees: number): void {
  if (avatarEntity == null || !Number.isFinite(degrees) || degrees === 0) return
  yaw = (((yaw + degrees) % 360) + 360) % 360
  Transform.getMutable(avatarEntity).rotation = Quaternion.fromEulerDegrees(0, yaw, 0)
}

function pollLoad(ctx: Ctx, dt: number): void {
  if (load == null || open == null || avatarEntity == null) return
  const { id, wave } = open
  if (load.drawnIn != null) {
    if (--load.drawnIn > 0) return
    load = null
    ctx.send({ kind: 'passportPreviewState', id, state: 'ready' })
    if (wave) playEmote(INTRO_EMOTE)
    return
  }
  const latest = latestLoad(avatarEntity)
  const state = latest != null && latest.timestamp > load.since ? latest.state : null
  if (state === LOADING_FINISHED) {
    load.drawnIn = DRAWN_FRAMES
    return
  }
  load.seconds += dt
  if (state === LOADING_FINISHED_WITH_ERROR || state === LOADING_NOT_FOUND || load.seconds >= LOAD_SECONDS) {
    load = null
    ctx.send({ kind: 'passportPreviewState', id, state: 'failed' })
  }
}

export function registerPassportPreview(ctx: Ctx): void {
  ctx.on('passportPreview', (msg) => {
    const next = msg.open
    if (next == null) {
      if (open == null) return
      open = null
      dispose()
      releasePreview()
      return
    }
    if (!(next.rect.width > 0 && next.rect.height > 0)) return
    // another passport's avatar starts over: facing front, loaded and waited for anew
    if (open != null && open.id !== next.id) dispose()
    open = { ...next, dpr: next.dpr > 0 ? next.dpr : 1 }
    holdPreview()
    if (avatarEntity == null) build(open)
    else fit(open)
    wear(open)
  })
  ctx.on('passportPreviewDrag', (msg) => {
    if (open?.id === msg.id) turn = dragTurn(turn, msg.dx, msg.seconds)
  })
  ctx.on('passportPreviewEmote', (msg) => {
    if (open?.id === msg.id) playEmote(msg.urn)
  })
  ctx.push((dt) => {
    if (open == null) return
    // a stage that came up under the passport since
    holdPreview()
    pollLoad(ctx, dt)
    const stepped = stepTurn(turn, dt)
    turn = stepped.turn
    turnBy(stepped.degrees)
  })
}

export function renderPassportPreview(): ReactEcs.JSX.Element | null {
  if (open == null || cameraEntity == null) return null
  const { rect, background } = open
  const scale = rect.height / COLUMN_HEIGHT
  return (
    // It takes the pointer, so nothing behind the passport's column does.
    <UiEntity
      uiTransform={{ positionType: 'absolute', position: { left: rect.x, top: rect.y }, width: rect.width, height: rect.height, overflow: 'hidden' }}
      uiBackground={{ texture: { src: BACKGROUNDS[background] }, textureMode: 'stretch' }}
      onMouseDown={() => {}}
    >
      <UiEntity
        uiTransform={{ positionType: 'absolute', position: { left: 0, top: PICTURE_TOP * scale }, width: rect.width, height: PICTURE_HEIGHT * scale }}
        uiBackground={{ videoTexture: { videoPlayerEntity: cameraEntity }, textureMode: 'stretch' }}
      />
    </UiEntity>
  )
}
