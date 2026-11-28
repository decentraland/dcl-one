// The typed API specs use. Every method is one console command (or a command and the wait that
// makes its effect observable), with the reply parsed as rendering/bevy-explorer/docs/guidelines/
// running.md ("Test commands") describes it. Specs never see which transport carries the line.
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { type Console, ConsoleError, quoteWord, redact, redactLine, replaceSecrets, secretsOf } from './console.ts'
import { type AuthIdentity, encodeIdentity } from './identity.ts'
import { type Selector, describeSelector, selectorArg, toSelectorObject } from './selectors.ts'
import { sleep, until } from './wait.ts'

/** `/photo_mode` and `/take_photo`: whether the in-world camera is open, where it is, the
 *  player's photo storage and this session's capture counters. */
export interface PhotoModeState {
  active: boolean
  requested: boolean | null
  position: [number, number, number]
  /** `[current, max]` photos kept, once `storageLoaded`. */
  storage: [number, number]
  storageLoaded: boolean
  phase: 'idle' | 'capturing' | string
  captures: number
  uploaded: number
  failed: number
  lastPhotoId: string | null
  lastError: string | null
  yaw?: number
  pitch?: number
  roll?: number
  fov?: number
  cursorLocked?: boolean
  hudHidden?: boolean
  source?: string
  unavailable?: Record<string, string>
}

/** What `/photo_mode` with a pose flag reports once the photo camera is placed: the state, with
 *  `placed` (Decentraland world coordinates, where `position` is the engine's). `settled` is false,
 *  with a `warning`, when photo mode did not open within `timeoutSeconds`, or a later request
 *  closed it first. */
export interface PhotoPoseState extends PhotoModeState {
  placed: [number, number, number] | null
  settled: boolean
  settleMs: number
  warning?: string
}

export interface WorldState {
  inWorld: boolean
  arrivals: number
  signedIn: boolean
  address: string | null
  guest: boolean
  realm: string
  realmConnected: boolean
  worldHeld: boolean
  outOfWorld: boolean | null
  position: [number, number, number] | null
  /** A text field (the HUD's or the engine's) holds the keyboard: keys are typed and resolve no
   *  action. Null where the client takes no input; absent from builds older than the suite, which
   *  the keyboard waits refuse. */
  keyboardClaimed?: boolean | null
}

/** A scene's lifecycle state as `/scene_loading_state` and the settling waits name it: `none` on a
 *  parcel with no scene, `refused` on one whose scene refuses the player (a scene or world ban; it
 *  is not loaded for them). */
export type SceneLifecycle = 'loading' | 'failed' | 'starting' | 'blocked' | 'broken' | 'running' | 'none' | 'refused'

/** What `/teleport --wait` and `/reload --wait` report: the scene settled (it runs, its content is
 *  shown and a frame with it was presented) or the wait ended, and why in `warning`. */
export interface SettleReport {
  /** null on a parcel with no scene or one whose scene refuses the player; absent when a
   *  teleport's wait ended before the landing */
  sceneHash?: string | null
  title?: string
  sceneState: SceneLifecycle
  /** scene ticks run during the wait */
  ticksWaited?: number
  /** frames drawn since the scene was ready: 2 when the reply settled on a ready scene (the first
   *  of them has been presented); 0 when it settled on a parcel with no scene or a refused one, or
   *  ended before the scene was ready */
  renderedFrames: number
  settleMs: number
  settled: boolean
  /** what holds a blocked scene (`frozen`, `gltfs loading`, ...) */
  blocked?: string[]
  warning?: string
}

export interface TeleportReport extends SettleReport {
  location: [number, number]
  realm?: string
  arrived: boolean
  sceneReady: boolean
}

export interface ReloadReport extends SettleReport {
  reloaded: true
  hint?: string
}

/** What `/scene_loading_state` reports: the parcel scene under the player, then counts of every
 *  scene by stage. */
export interface SceneStateReport {
  /** null until the player has a position */
  playerParcel: [number, number] | null
  /** the player has not landed yet (the scene under them loads first); null until the player has
   *  a position */
  outOfWorld: boolean | null
  /** the parcel's scene, known once its pointer has resolved, before the scene spawns; null on a
   *  parcel with no scene or one whose scene refuses the player, or until then */
  sceneHash: string | null
  state: SceneLifecycle
  /** the scene readiness `/teleport --wait` and `/reload --wait` wait for: `state` is `running`.
   *  A teleport's wait also needs the player landed (`outOfWorld` false) */
  isReady: boolean
  /** the load stage of the scene under the player, or why there is none */
  stage: string
  /** what holds the scene under the player (`frozen`, `gltfs loading`, ...), sorted */
  blockedBy: string[]
  /** why each null reading above is null */
  unavailable: Record<string, string>
  /** the player's own scene is still loading, which holds the others back */
  current: boolean
  spawned: number
  definition: number
  crdt: number
  javascript: number
  failed: number
  running: number
  blocked: number
  broken: number
  gltf: number
  preloads: number
  texturePreloads: number
  /** `--wait` commands still waiting for a scene to settle */
  settleWaits: number
}

/**
 * What `/hover_entity` and `/click_entity` report. A hit on an avatar or on another scene's
 * entity reports `entity` and `container` as 4294967295.
 */
export interface PointerReport {
  hit: { entity: number; container: number; scene: string; playerDistance: number; cameraDistance: number } | null
  entries: { button: string; event: string; hoverText: string | null; enabled: boolean; tooFar: boolean }[]
  events: { event: string; button: string; tick: number }[]
  diagnosis: string | null
}

/** `/raycast_stats`: the capped tier's budget and last pass, and the uncapped tier's last pass. */
export interface RaycastStats {
  budgetMs: number
  ran: number
  deferred: number
  spentMs: number
  uncappedRan: number
  uncappedSpentMs: number
}

/** A `/media_state` player row; the retry fields are the web backend's. */
export interface MediaPlayerRow {
  type: 'video' | 'audio'
  entity: string
  scene: string | null
  source: string
  lastPlaybackState?: string | null
  textureSize?: [number, number] | null
  retryAttempt?: number | null
  retryFailures?: number | null
  retryDelaySeconds?: number | null
  retryGaveUp?: boolean | null
  lastError?: string | null
}

/** `/media_state`: the loaded media players. */
export interface MediaState {
  backend: string
  players: MediaPlayerRow[]
}

/** `/media_retry_delay`: the wait before a failed web source's first reopen and the reopens allowed. */
export interface MediaRetryDelay {
  firstDelayMs: number
  maxReopens: number
}

export interface Control {
  found: true
  role: string
  name: string
  text: string
  value?: string
  checked?: boolean
  disabled: boolean
  visible: boolean
  actionable: boolean
  reason?: string
  /** Inside a region that says it is still loading (`aria-busy`): what it shows is not final. */
  busy?: boolean
  x: number
  y: number
  width: number
  height: number
}

export interface NotFound {
  found: false
  error?: string
}

export type HudRead = Control | NotFound

export type InputKind = 'trusted' | 'synthetic'

export interface HudInput extends Control {
  input: InputKind
}

export interface HudEntry {
  role: string
  name: string
}

export type HudState = 'visible' | 'hidden' | 'enabled' | 'actionable' | 'ready'

export interface UserData {
  name: string
  address: string
  version: number
  web3: boolean
}

export interface RenderedWearable {
  category: string
  urn: string
}

export interface EmoteState {
  urn: string
  source: 'velocitySelected' | 'triggeredEmote' | 'sceneMovementAnim'
  repeat: boolean
  finished: boolean
  mask: 'fullBody' | 'upperBody'
  /** An idle pose, which a triggered emote takes over: a scene's movement animation outranks one
   *  only while it is not idle. Absent from builds before it was reported. */
  idle?: boolean
}

export interface AvatarReport {
  kind: 'player' | 'otherPlayer' | 'sceneAvatar' | 'preview' | 'other'
  address: string | null
  selected: { bodyShape: string | null; wearables: string[]; emotes: string[]; forceRender: string[]; fromScene: boolean }
  rendered: { processed: boolean; bodyShape: string; wearables: RenderedWearable[]; hidden: string[] } | null
  emote: EmoteState | null
  upperBodyEmote: EmoteState | null
  /** The files the avatar could not be built from; `noBody` when neither its body nor the default
   *  body could be. Absent from builds before it was reported. */
  loadFailures?: { bodies: string[]; wearables: string[]; noBody: boolean }
}

export interface ChatMessage {
  timestamp: number
  channel: string
  message: string
  address: string | null
  messageId: string | null
}

export interface ChatHistory {
  total: number
  messages: ChatMessage[]
}

/** A nametag on show (`/nametag_state` `tags`). `bubble` never carries the message. */
export interface NametagTag {
  /** An address, `local`, or null for an avatar with no wallet. */
  player: string | null
  /** The tag is over an avatar a scene made (`AvatarShape`): its suffix is the end of the avatar's id. */
  sceneAvatar: boolean
  /** False for a tag that is its plate alone: the avatar has no name, or a scene's area hides it. */
  nameShown: boolean
  /** The plate a scene put over the name with `AvatarNametag`; its colours are `#RRGGBB`. */
  plate: { label: string; labelColor: string; backgroundColor: string; borderColor: string } | null
  /** Empty, as `suffix` is, for an avatar with no name. */
  name: string
  suffix: string
  verified: boolean
  official: boolean
  voice: 'off' | 'idle' | 'speaking' | 'hushed'
  /** Metres from the camera; null until the tag is first placed. */
  distance: number | null
  opacity: number
  /** Metres per tag unit. */
  scale: number
  position: [number, number, number]
  /** Metres over the head bone for an avatar at a scale of one; null until the avatar is built. */
  headOffset: number | null
  bubble: { kind: 'nearby' | 'community' | 'dm'; mention: boolean; length: number } | null
}

export interface NametagState {
  visible: boolean
  shown: number
  /** Of those shown, the tags over a scene's avatars. */
  sceneAvatars: number
  /** Of those shown, the tags with a scene's plate. */
  plates: number
  /** Tags whose names don't show: out of range, blocked, or hidden by the scene or a setting. */
  hidden: number
  /** The glyph atlas' distance-field and colour page sizes, in pixels. */
  atlas: [number, number]
  bubbles: 'All' | 'NearbyOnly' | 'None'
  tags: NametagTag[]
}

/** A participant of the voice call or community stream the player is in (`/voice_state` `call`). */
export interface VoiceCallParticipant {
  address: string
  name: string
  role: string
  isSpeaker: boolean
  isRequestingToSpeak: boolean
  isMuted: boolean
  isSpeaking: boolean
}

/** `/voice`'s actions: ring (`call`), answer (`accept` / `reject`), hang up or leave (`end`), open,
 *  join or close a community stream, raise the hand, the call mic, and moderation (`deny` turns
 *  down a raised hand). */
export type VoiceAction =
  | ['call', string]
  | ['accept' | 'reject' | 'end' | 'close']
  | ['start' | 'join', string]
  | ['hand', 'up' | 'down']
  | ['mic', 'on' | 'off']
  | ['promote' | 'demote' | 'kick' | 'deny', string]

/** The private call or community stream as the HUD sees it: its status
 *  (`disconnected`, `receivedCall`, `startingCall`, `startedCall`, `inCall`, `endingCall`,
 *  `rejectingCall`, `busy`, `genericError`) and kind (`none`, `private`, `community`). */
export interface VoiceCallState {
  status: string
  kind: string
  callId: string | null
  communityId: string | null
  peer: string | null
  connection: string
  participants: VoiceCallParticipant[]
  micEnabled: boolean
  localSpeaker: boolean
  localRole: string
  localRequestingToSpeak: boolean
  error: string | null
  /** `error` tells how the call ended (hung up, removed, joined elsewhere), not a failure. */
  errorIsNotice: boolean
}

/** `/community_state`: community chat's routing and counters (no message bodies). */
export interface CommunityState {
  routerIdentity: string
  routerInRoom: boolean
  channels: string[]
  channelCount: number
  roomSpawned: boolean
  sent?: number
  received?: number
  reactionsSent?: number
  reactionsReceived?: number
  dropped?: { notMember: number; unattributed: number; rateLimited: number }
  lastError?: string | null
}

export type RoomState = 'connecting' | 'connected' | 'reconnecting' | 'disconnected' | 'failed'
  | 'paused' | 'denied' | 'invalid' | 'identity_conflict' | 'credentials_required'

export interface VoiceState {
  mic: { available: boolean; enabled: boolean; sceneBlocked: boolean }
  livekit: boolean
  connected: boolean | null
  rooms: { scene: boolean; state: RoomState }[] | null
  speakers: string[] | null
  /** Null without `livekit`. */
  call: VoiceCallState | null
}

/** The saved mutes for nearby voice (`/nearby_mutes`). */
export interface NearbyMutes {
  /** Null while signed out. */
  account: string | null
  load: 'idle' | 'loading' | 'loaded' | 'failed'
  count: number
  /** Lowercased addresses, sorted; the first 32. */
  muted: string[]
}

export interface ConnectivityState {
  connectionLost: boolean
  probing: boolean
  everConnected: boolean
  suppressed: boolean
  rooms: { scope: string; state: RoomState; retryAt: number | null }[]
}

/** `/ban_block_state [address]`: the one answer to "is this user banned or blocked?", with every
 *  `null` reading explained in `unavailable` by its field path. */
export interface BanBlockState {
  address: string | null
  blocks: {
    state: 'unavailable' | 'live'
    reason: 'social_feature_off' | 'not_signed_in' | 'social_not_connected' | null
    blocked: string[]
    blockedBy: string[]
  }
  platformBan: {
    state: 'unknown' | 'checked' | 'failed'
    isBanned: boolean | null
    source: 'ban_status_endpoint' | 'scene_admission_refusal' | null
    reason: string | null
    matchedOn: string | null
    customMessage: string | null
    expiresAt: string | null
    checkedAgoSecs: number | null
    error: string | null
  }
  sceneAdmission: {
    state: 'unknown' | 'allowed' | 'refused'
    scope: 'platform' | 'world' | 'scene' | null
    message: string | null
    sceneId: string | null
    /** Known refusals in this wallet/realm session; retryDue is elapsed engine seconds. */
    retainedRefusals: { sceneId: string; scope: 'world' | 'scene'; retryDue: number }[]
  }
  sceneRoom: {
    state: 'not_connected' | 'connected'
    sceneId: string | null
    bannedAddresses: string[]
    sceneAdmins: string[]
  }
  lookup: {
    address: string
    isBlockedEitherWay: boolean
    direction: 'none' | 'local_user_blocked_them' | 'they_blocked_local_user' | 'mutual'
    bannedInScene: boolean
    adminPermissions: 'admin' | 'not_admin' | 'not_loaded_yet'
  } | null
  unavailable: Record<string, string>
}

export interface ScreenPosition {
  entity: string
  scene: string
  anchor: 'origin' | 'bounds'
  x: number | null
  y: number | null
  onScreen: boolean
  inFront: boolean
  depth: number
  distance: number
  viewport: { x: number; y: number; width: number; height: number }
  /** `x`/`y` as 0 to 1 of the window from the top left, as `clickAt(..., { normalized: true })` takes them */
  normalized: { x: number; y: number } | null
  screen: Screen
}

/** The window a coordinate is read against, in the logical pixels the commands use; `scale` is
 *  the device pixel ratio, so a screenshot's pixels are `scale` times as many. */
export interface Screen {
  width: number
  height: number
  scale: number
}

/** Where `/click_at` clicked, in pixels and normalized, and on what window. The web page adds how
 *  (`input`: `trusted` from the test runner, else `synthetic`). */
export interface ClickAtReply {
  x: number
  y: number
  normalized: { x: number; y: number }
  button: MouseButton
  screen: Screen
  input?: 'trusted' | 'synthetic'
}

export interface Position {
  x: number
  y: number
  z: number
}

/** What `/move_player_to --wait` reports once the player has landed and been still for 0.25 s, or
 *  once `timeoutSeconds` ran out (`settled: false` and a `warning` naming what it waited for: the
 *  move, the landing or the stillness). `position` is Decentraland
 *  world coordinates. */
export interface MoveReport {
  position: [number, number, number] | null
  parcel: [number, number] | null
  settled: boolean
  settleMs: number
  warning?: string
}

/** The player camera's zoom level, as `/camera_mode` sets it. */
export type CameraLevel = 'first_person' | 'third_person' | 'drone'

/** What `/camera_mode` reports: the level before and after, and whether it changed. */
export interface CameraModeChange {
  before: CameraLevel
  after: CameraLevel
  changed: boolean
}

export interface LiveScene {
  title: string
  hash: string
  portable: boolean
  smartWearable: boolean
  broken: boolean
}

export type SmartWearableState = 'loading' | 'running' | 'blocked' | 'stopped' | 'notSmart' | 'failed'

export interface SmartWearableEntry {
  urn: string
  state: SmartWearableState
  reason?: string
}

export interface SmartWearables {
  /** Whether the parcel scene the player stands in allows portable experiences; null when there
   *  are no smart wearables (the reply then has no header). */
  currentSceneAllows: boolean | null
  entries: SmartWearableEntry[]
}

export type SmartWearableAction = 'run' | 'kill' | 'restart'

const SMART_WEARABLE_STATES: readonly SmartWearableState[] = ['loading', 'running', 'blocked', 'stopped', 'notSmart', 'failed']

export type PortableKind = 'scene' | 'smartWearable'
export type PortableState = 'loading' | 'running' | 'failed' | 'blocked' | 'stopped'

/** A portable experience the HUD lists: every portable but startup ones and the HUD's own scene,
 *  and smart wearables that are stopped while still equipped. */
export interface Portable {
  name: string
  /** The live scene hash `/stop_portable` takes; null for a smart wearable that is not loaded. */
  hash: string | null
  kind: PortableKind
  state: PortableState
}

const PORTABLE_STATES: readonly PortableState[] = ['loading', 'running', 'failed', 'blocked', 'stopped']

/** `/live_scenes`: `<title> [<hash>, portable, smart wearable, broken]` per line, the flags
 *  optional, or `no scenes loaded`. */
export function parseLiveScenes(reply: string): LiveScene[] {
  if (reply.trim() === 'no scenes loaded') return []
  return reply
    .split('\n')
    .filter((line) => line.trim() !== '')
    .map((line) => {
      const match = /^(.*) \[([^,\]]+)((?:, [a-z ]+)*)\]$/.exec(line.trim())
      if (!match) throw new Error(`unparseable live_scenes line: ${line}`)
      const flags = match[3]!.split(',').map((flag) => flag.trim()).filter(Boolean)
      return { title: match[1]!, hash: match[2]!, portable: flags.includes('portable'), smartWearable: flags.includes('smart wearable'), broken: flags.includes('broken') }
    })
}

/** `/smart_wearables`: `current scene allows: <bool>`, then `<urn> <state> [<reason>]` per entry,
 *  or `no smart wearables`. */
export function parseSmartWearables(reply: string): SmartWearables {
  const lines = reply.split('\n').map((line) => line.trim()).filter(Boolean)
  if (lines.length === 1 && lines[0] === 'no smart wearables') return { currentSceneAllows: null, entries: [] }
  const header = /^current scene allows: (true|false)$/.exec(lines[0] ?? '')
  if (!header) throw new Error(`unparseable smart_wearables header: ${reply.slice(0, 200)}`)
  const entries = lines.slice(1).map((line): SmartWearableEntry => {
    const match = /^(\S+) (\S+)(?: (.+))?$/.exec(line)
    const state = match?.[2] as SmartWearableState | undefined
    if (!match || !state || !SMART_WEARABLE_STATES.includes(state)) throw new Error(`unparseable smart_wearables line: ${line}`)
    const reason = match[3]?.replace(/^\[(.*)\]$/, '$1')
    return reason ? { urn: match[1]!, state, reason } : { urn: match[1]!, state }
  })
  return { currentSceneAllows: header[1] === 'true', entries }
}

const SMART_WEARABLE_REPLIES: Record<SmartWearableAction, string> = { run: 'running', kill: 'killed', restart: 'restarted' }

/** `/portables`: `<name> [<hash or ->, <kind>, <state>]` per line, or `no portables`. The kind is
 *  `scene` or `smartWearable` (`smart wearable` reads the same). */
export function parsePortables(reply: string): Portable[] {
  if (reply.trim() === 'no portables') return []
  return reply
    .split('\n')
    .filter((line) => line.trim() !== '')
    .map((line): Portable => {
      const match = /^(.*) \[([^,\]]+), ([^,\]]+), ([^,\]]+)\]$/.exec(line.trim())
      const kind = match?.[3]!.replace(/ (\w)/g, (_, c: string) => c.toUpperCase()) as PortableKind | undefined
      const state = match?.[4] as PortableState | undefined
      if (!match || (kind !== 'scene' && kind !== 'smartWearable') || !state || !PORTABLE_STATES.includes(state)) throw new Error(`unparseable portables line: ${line}`)
      return { name: match[1]!, hash: match[2] === '-' ? null : match[2]!, kind, state }
    })
}

/** What `/hud_login` answers once the sign-in has landed and the welcome page is done: the
 *  account, and the HUD's phase: `lobby`, `entering` (on its way to the destination), `picking`
 *  or `world`. */
export interface HudLoginReply {
  address: string
  guest: boolean
  phase: 'lobby' | 'picking' | 'entering' | 'world'
}

export interface CommandRecord {
  /** The line as sent, with the payload of `/login_identity` or `/hud_login` redacted. */
  line: string
  startedAt: number
  ms: number
  reply?: string
  error?: string
}

export type CommandListener = (record: CommandRecord) => void

export interface ExplorerOptions {
  /** Accept `"input": "synthetic"` from the HUD commands (a run without the web driver). Off by
   *  default: a spec that passed on DOM events proves nothing about the browser's input path. */
  allowSyntheticInput?: boolean
  /** The catalyst's window between two deployments of one profile (`PROFILE_DEPLOY_WINDOW_MS`). */
  profileDeployWindowMs?: number
  /** How many deployments of one profile go through in a span (`PROFILE_DEPLOY_BURST`). */
  profileDeployBurst?: { count: number; spanMs: number }
}

/** The catalyst takes one deployment of a profile per window: the reference catalyst's rate limit,
 *  3 s per address, which zone's peers apply (a second one inside it fails with `Entity rate
 *  limited`, and the HUD shows "failed to deploy"), with a margin for the deployment going out
 *  after the engine reports the new version. */
export const PROFILE_DEPLOY_WINDOW_MS = 4_000

/** And no more than five deployments of a profile in a minute: zone's peers answer a sixth with
 *  `429 Too many requests` and no `Retry-After` (seen on runs that deployed at 0, 5, 14, 27, 32
 *  and 38 s), which the engine does not retry and the HUD shows as "Couldn't save your look". */
export const PROFILE_DEPLOY_BURST = { count: 5, spanMs: 61_000 }

/** How long the engine may take to hand the keyboard to a HUD text field, or to take it back once
 *  the field lets it go: the HUD relays its focus through the bridge scene, which takes a few
 *  frames. A ceiling: the wait ends as soon as `/world_state` reports the change. */
export const KEYBOARD_TIMEOUT_MS = 10_000

export type MouseButton = 'left' | 'right' | 'middle'

/** `/screenshot` options; any of them makes the reply JSON (`CapturedShot`). The engine has no
 *  flag for the JSON reply alone, so `screenshot({})` sends `--max-width 1920`: a frame wider
 *  than 1920 px (a HiDPI window) comes back scaled down, and `frame` names the size captured.
 *  The bare `screenshot()` is the full-size PNG. */
export interface ScreenshotOptions {
  /** 64 to 1920 */
  maxWidth?: number
  jpg?: boolean
  /** hides the HUD and the scenes' window UI for the frame */
  worldOnly?: boolean
}

interface Size {
  width: number
  height: number
}

/** A `/screenshot` with options: the image and what it was scaled from. */
export interface CapturedShot {
  /** `640x325 (screen 1600x813, frame 3200x1626) full-view capture at parcel (149,149)[, waited N ms for the camera blend]` */
  caption: string
  format: 'png' | 'jpg'
  width: number
  height: number
  frame: Size
  screen: (Size & { scale: number }) | null
  parcel: { x: number; y: number } | null
  worldOnly: boolean
  blendWaitMs: number
  image: Buffer
  info?: string
}

export interface CaptureSequenceOptions extends ScreenshotOptions {
  /** 1 to 20; 5 when not given */
  count?: number
  /** 0.05 to 2, default 0.5 */
  intervalSeconds?: number
  /** native only: the folder the engine writes to; give each sequence its own, or read
   *  `staleFrames` */
  dir?: string
  /** where frames kept in the engine's memory (the web client's) are written; `savedTo` names each file */
  saveTo?: string
}

export interface CapturedFrame {
  index: number
  /** a file natively, `capture://<sequence>/frame-<index>.<ext>` in the web client's memory */
  path: string
  width: number
  height: number
  bytes: number
  atSeconds: number
  intervalFromPreviousSeconds: number | null
  engineFrame: number
  /** the scene's tick when the frame was rendered */
  tick: number | null
  late: boolean
  savedTo?: string
}

/** A `/screenshot --count` reply: paths, never images. */
export interface CaptureSequence {
  directory: string
  stored: 'disk' | 'memory'
  count: number
  format: 'png' | 'jpg'
  worldOnly: boolean
  requestedIntervalSeconds: number
  meanIntervalSeconds: number | null
  late: number
  screen: (Size & { scale: number }) | null
  parcel: { x: number; y: number } | null
  frames: CapturedFrame[]
  note: string
  info?: string
  /** frame files in `directory` this sequence did not write (a reused `dir`); absent when none */
  staleFrames?: number
  warning?: string
}

/** `/screenshot --state`: the capture gate. */
export interface CaptureState {
  holder: 'screenshot' | 'screenshot --count' | 'none'
  framesTaken: number
  count: number
  worldOnly: boolean
  framesInMemory: number
  bytesInMemory: number
}

function captureFlags(options: ScreenshotOptions): string {
  return `${options.maxWidth === undefined ? '' : ` --max-width ${options.maxWidth}`}${options.jpg ? ' --jpg' : ''}${options.worldOnly ? ' --world-only' : ''}`
}

/** Writes the frames a sequence kept in the engine's memory (`stored: "memory"`) into `dir`, read
 *  with `/screenshot --frame`, and names each file in its `savedTo`. A sequence on disk is returned as is. */
export async function saveCaptureFrames(explorer: Pick<Explorer, 'run'>, sequence: CaptureSequence, dir: string): Promise<CaptureSequence> {
  if (sequence.stored !== 'memory') return sequence
  await mkdir(dir, { recursive: true })
  const frames: CapturedFrame[] = []
  for (const frame of sequence.frames) {
    const file = path.join(dir, path.posix.basename(frame.path))
    await writeFile(file, Buffer.from(await explorer.run(`/screenshot --frame ${quoteWord(frame.path)}`), 'base64'))
    frames.push({ ...frame, savedTo: file })
  }
  return { ...sequence, frames }
}

/** Why a client whose `/world_state` has no `keyboardClaimed` cannot run the scenarios. */
export const OLDER_CLIENT =
  "the client's /world_state has no keyboardClaimed: it is older than this suite, which waits on it; run the suite against a web build of its own commit"

const DEFAULT_HUD_TIMEOUT_MS = 10_000
/** How long a click, a fill or a hover waits for its control to take a click: uncovered and no
 *  longer moving (a page slides and fades in, a button grows in on hover). */
const ACTIONABLE_TIMEOUT_MS = 10_000

const DEFAULT_ARRIVAL_TIMEOUT_MS = 180_000
/** A transport's command timeout when it names none (the fake). */
const DEFAULT_COMMAND_TIMEOUT_MS = 120_000
/** What a settling command's wait inside the engine stays under the transport's command timeout
 *  by, so a slow load is reported by the engine rather than cut by the transport. */
const SETTLE_MARGIN_MS = 20_000
/** `--timeout-seconds` of `--wait` when the flag is left out. */
const ENGINE_SETTLE_MS = 60_000
/** The longest `--timeout-seconds` the engine takes. */
const ENGINE_MAX_SETTLE_MS = 300_000

/** The error as it may be rethrown: a transport's own error (a timeout, a closed driver) can
 *  quote the line, and the test runner prints the message and the stack. */
function scrubbed(line: string, error: unknown): unknown {
  if (!(error instanceof Error)) return error
  error.message = redact(line, error.message)
  if (error.stack !== undefined) error.stack = redact(line, error.stack)
  return error
}

export class Explorer {
  readonly console: Console
  readonly hud: Hud
  /** When this client's driver was made: before its sign-in, so before anything the sign-in did. */
  readonly openedAt = Date.now()
  readonly #listeners = new Set<CommandListener>()
  readonly #allowSynthetic: boolean
  readonly #deployWindowMs: number
  readonly #deployBurst: { count: number; spanMs: number }
  /** When each recent deployment of the own profile was seen, oldest first. */
  readonly #deploymentsSeenAt: number[] = []
  /** The own profile's version as last read, and when it was last seen to change. */
  #ownVersion: number | undefined
  #ownVersionChangedAt = 0
  /** The credentials the lines run so far carried (`secretsOf`), for text that may quote them. */
  readonly #secrets = new Set<string>()

  constructor(console: Console, options: ExplorerOptions = {}) {
    this.console = console
    this.#allowSynthetic = options.allowSyntheticInput ?? false
    this.#deployWindowMs = options.profileDeployWindowMs ?? PROFILE_DEPLOY_WINDOW_MS
    this.#deployBurst = options.profileDeployBurst ?? PROFILE_DEPLOY_BURST
    this.hud = new Hud(this, this.#allowSynthetic)
  }

  /** Observe every command sent and its reply or failure (the report records them). */
  onCommand(listener: CommandListener): () => void {
    this.#listeners.add(listener)
    return () => this.#listeners.delete(listener)
  }

  /** Run a raw console line. */
  async run(line: string): Promise<string> {
    const startedAt = Date.now()
    const record: CommandRecord = { line: redactLine(line.trim()), startedAt, ms: 0 }
    for (const secret of secretsOf(line)) this.#secrets.add(secret)
    try {
      const reply = await this.console.run(line)
      record.ms = Date.now() - startedAt
      // redacted before it is cut, so a cut cannot leave a part of the credential
      const safe = redact(line, reply)
      record.reply = safe.length > 4000 ? `${safe.slice(0, 4000)}... (${safe.length} chars)` : safe
      return reply
    } catch (error) {
      record.ms = Date.now() - startedAt
      const safe = scrubbed(line, error)
      record.error = redact(line, safe instanceof Error ? safe.message : String(safe))
      throw safe
    } finally {
      for (const listener of this.#listeners) listener(record)
    }
  }

  /** `text` with every credential a line of this client carried replaced: the page's console,
   *  which the HUD may have written an error into. */
  redactSecrets(text: string): string {
    return replaceSecrets(this.#secrets, text)
  }

  /** Run a line whose reply is JSON. A reply that does not parse is an error naming the command,
   *  never silently a string. */
  async json<T>(line: string): Promise<T> {
    const reply = await this.run(line)
    try {
      return JSON.parse(reply) as T
    } catch {
      throw new Error(`\`${redactLine(line)}\` did not reply JSON: ${redact(line, reply).slice(0, 200)}`)
    }
  }

  // --- world --------------------------------------------------------------------------------

  worldState(): Promise<WorldState> {
    return this.json<WorldState>('/world_state')
  }

  // --- photo mode ---------------------------------------------------------------------------

  /** Photo mode (the camera reel's in-world camera) as `/photo_mode` reports it; `state` opens or
   *  closes it first (the change lands a frame later: `waitPhotoMode`). */
  photoMode(state?: 'on' | 'off'): Promise<PhotoModeState> {
    return this.json<PhotoModeState>(state ? `/photo_mode ${state}` : '/photo_mode')
  }

  /** Places the photo camera (opening photo mode when it is closed): `position` and `lookAt` are
   *  Decentraland world coordinates, `position` within 50 m of the player, `fov` vertical degrees
   *  (10 to 120). Resolves once it is there, or after `timeoutSeconds` (5 by default) with
   *  `settled: false`. A pose photo mode cannot take rejects with `{"refused", "cause"}` as the
   *  error's output. */
  placePhotoCamera(pose: {
    position?: [number, number, number]
    lookAt?: [number, number, number]
    fov?: number
    timeoutSeconds?: number
  }): Promise<PhotoPoseState> {
    let line = '/photo_mode'
    if (pose.position) line += ` --position ${pose.position.join(' ')}`
    if (pose.lookAt) line += ` --look-at ${pose.lookAt.join(' ')}`
    if (pose.fov !== undefined) line += ` --fov ${pose.fov}`
    if (pose.timeoutSeconds !== undefined) line += ` --timeout-seconds ${pose.timeoutSeconds}`
    return this.json<PhotoPoseState>(line)
  }

  waitPhotoMode(active: boolean, timeoutMs = 5000): Promise<PhotoModeState> {
    return until(() => this.photoMode(), (state) => state.active === active, {
      timeoutMs,
      what: `photo mode to be ${active ? 'on' : 'off'}`
    })
  }

  waitPhotoState(ready: (state: PhotoModeState) => boolean, what: string, timeoutMs = 10_000): Promise<PhotoModeState> {
    return until(() => this.photoMode(), ready, { timeoutMs, what })
  }

  /** Delete one of the player's camera-reel photos (`/delete_photo`), and wait for the storage
   *  count to drop: a failure lands in `lastError`. */
  async deletePhoto(id: string, timeoutMs = 20_000): Promise<void> {
    const before = await this.photoMode()
    await this.run(`/delete_photo ${id}`)
    await until(() => this.photoMode(), (state) => state.storage[0] < before.storage[0] || state.lastError !== before.lastError, {
      timeoutMs,
      what: `the photo ${id} to be deleted`
    })
    const after = await this.photoMode()
    if (after.storage[0] >= before.storage[0]) throw new Error(`the photo ${id} was not deleted: ${after.lastError}`)
  }

  /** Take a photo in photo mode; rejects with the engine's reason when it cannot start. */
  takePhoto(): Promise<PhotoModeState> {
    return this.json<PhotoModeState>('/take_photo')
  }

  /** Wait until the player is in the world (signed in, realm connected, not out of world). */
  waitInWorld(timeoutMs = DEFAULT_ARRIVAL_TIMEOUT_MS): Promise<WorldState> {
    return until(() => this.worldState(), (state) => state.inWorld, { timeoutMs, what: 'the player to be in the world' })
  }

  /** Teleport to a parcel and wait inside the engine for the arrival and the scene:
   *  `/teleport --wait` replies once the player has landed and the parcel's scene settled, or its
   *  wait ended (`teleport.settled` false, `teleport.warning` says why), and the world state is
   *  read once after it. Only a wait the engine ended before the landing (it waits less than the
   *  transport's command timeout) is followed by a poll for `arrivals` to grow and `inWorld`, for
   *  what is left of `timeoutMs`. */
  async teleport(x: number, y: number, options: { timeoutMs?: number } = {}): Promise<WorldState & { teleport: TeleportReport }> {
    const timeoutMs = options.timeoutMs ?? DEFAULT_ARRIVAL_TIMEOUT_MS
    const seconds = this.#settleSeconds(timeoutMs)
    const startedAt = Date.now()
    const before = (await this.worldState()).arrivals
    const teleport = await this.json<TeleportReport>(`/teleport ${x} ${y} --wait --timeout-seconds ${seconds}`)
    if (teleport.arrived) return { ...(await this.worldState()), teleport }
    const left = Math.max(0, timeoutMs - (Date.now() - startedAt))
    return { ...(await this.#waitArrival(before, left, `the arrival at ${x},${y}`)), teleport }
  }

  /** Reload the parcel scene under the player and wait for the new one to settle
   *  (`/reload --current --wait`); `settled` false with a `warning` when it did not in time. */
  async reload(options: { timeoutMs?: number } = {}): Promise<ReloadReport> {
    // the engine's own default, unless the transport would cut it
    const timeout = options.timeoutMs === undefined && this.#maxSettleMs() >= ENGINE_SETTLE_MS ? '' : ` --timeout-seconds ${this.#settleSeconds(options.timeoutMs ?? ENGINE_SETTLE_MS)}`
    return this.json<ReloadReport>(`/reload --current --wait${timeout}`)
  }

  /** The longest a settling command waits inside the engine: under the transport's command timeout. */
  #maxSettleMs(): number {
    return Math.max(1000, (this.console.commandTimeoutMs ?? DEFAULT_COMMAND_TIMEOUT_MS) - SETTLE_MARGIN_MS)
  }

  /** `--timeout-seconds` for a wait of `timeoutMs`, inside the engine's range (above 0, at most
   *  300 s) and under the transport's command timeout. A `timeoutMs` that is not above 0 is refused
   *  here, as the engine would. */
  #settleSeconds(timeoutMs: number): number {
    if (!(timeoutMs > 0)) throw new RangeError(`a settling wait needs a timeout above 0 ms; got ${timeoutMs}`)
    return Math.min(timeoutMs, this.#maxSettleMs(), ENGINE_MAX_SETTLE_MS) / 1000
  }

  /** The parcel scene under the player (its hash, state, what blocks it and whether it is ready)
   *  and the counts of every scene by load stage. */
  sceneState(): Promise<SceneStateReport> {
    return this.json<SceneStateReport>('/scene_loading_state')
  }

  /** Change realm (a realm URL or a world name) and wait for the arrival in it. */
  async changeRealm(realm: string, options: { timeoutMs?: number } = {}): Promise<WorldState> {
    const before = (await this.worldState()).arrivals
    await this.run(`/changerealm ${realm}`)
    return this.#waitArrival(before, options.timeoutMs ?? DEFAULT_ARRIVAL_TIMEOUT_MS, `the arrival in ${realm}`)
  }

  #waitArrival(before: number, timeoutMs: number, what: string): Promise<WorldState> {
    return until(() => this.worldState(), (state) => state.arrivals > before && state.inWorld, { timeoutMs, what })
  }

  /** Walk to a world position through the movement controller; resolves on arrival (0.5 m). */
  async walkTo(x: number, y: number, z: number, timeoutS = 20): Promise<void> {
    await this.run(`/walk_player_to ${x} ${y} ${z} ${timeoutS}`)
  }

  /** Move straight to a world position (Decentraland coordinates) as a scene's `movePlayerTo`
   *  does: `cameraTarget` turns the camera along the line from the new position (the feet) to a
   *  point and `avatarTarget` turns the avatar to face one (the camera target without it). Resolves once the player has landed and is still, or after
   *  `timeoutSeconds` (10 by default) with `settled: false`. The engine has `/move_player_to` in
   *  preview only. */
  moveTo(
    x: number,
    y: number,
    z: number,
    options: { cameraTarget?: [number, number, number]; avatarTarget?: [number, number, number]; timeoutSeconds?: number } = {}
  ): Promise<MoveReport> {
    let line = `/move_player_to ${x} ${y} ${z}`
    if (options.cameraTarget) line += ` --camera-target ${options.cameraTarget.join(' ')}`
    if (options.avatarTarget) line += ` --avatar-target ${options.avatarTarget.join(' ')}`
    line += ' --wait'
    if (options.timeoutSeconds !== undefined) line += ` --timeout-seconds ${options.timeoutSeconds}`
    return this.json<MoveReport>(line)
  }

  /** Sets the player's zoom level, as zooming does; the camera then blends to it. Rejects with
   *  `{"refused", "cause"}` as the error's output while photo mode (`cause.holder` `photo_mode`)
   *  or a scene (`cause.holder` `scene`) holds the camera. */
  setCameraMode(level: CameraLevel): Promise<CameraModeChange> {
    return this.json<CameraModeChange>(`/camera_mode ${level}`)
  }

  /** `/player_position`, parsed: DCL axes, so the parcel is `floor(x / 16), floor(z / 16)`. */
  async position(): Promise<Position> {
    const reply = await this.run('/player_position')
    const match = /\(\s*(-?[\d.]+)\s*,\s*(-?[\d.]+)\s*,\s*(-?[\d.]+)\s*\)/.exec(reply)
    if (!match) throw new Error(`unparseable player_position: ${reply}`)
    return { x: Number(match[1]), y: Number(match[2]), z: Number(match[3]) }
  }

  // --- session ------------------------------------------------------------------------------

  /** Sign in with an identity (while the world is held: the lobby, before any realm). `name` and
   *  `look` apply to a first login only. */
  async loginIdentity(identity: AuthIdentity | string, options: { name?: string; look?: number; guest?: boolean; defaultOnError?: boolean } = {}): Promise<void> {
    const payload = typeof identity === 'string' ? identity : encodeIdentity(identity)
    const flags: string[] = []
    if (options.defaultOnError) flags.push('--default-on-error')
    if (options.guest) flags.push('--guest')
    if (options.name !== undefined) flags.push(`--name ${options.name}`)
    if (options.look !== undefined) flags.push(`--look ${options.look}`)
    await this.run(['/login_identity', payload, ...flags].join(' '))
  }

  /** Sign in through the HUD's own flow (`/hud_login`, the web only): the identity becomes the
   *  page's stored sign-in and goes in as its Jump In does, so the HUD leaves its start page and
   *  accepts the terms its welcome page asks for; `name` and `look` pick a first login's profile
   *  (an existing one keeps its own); `realm` and `position` are picked from the lobby as a link's
   *  destination would be, which releases the held world. Resolves once the sign-in has landed
   *  and the page is done, not once the world is reached. */
  async hudLogin(
    identity: AuthIdentity | string,
    options: { name?: string; look?: number; guest?: boolean; defaultOnError?: boolean; realm?: string; position?: string } = {}
  ): Promise<HudLoginReply> {
    const payload = typeof identity === 'string' ? identity : encodeIdentity(identity)
    const flags: string[] = []
    // the line splits on whitespace: a value with any would read as stray words
    const flag = (name: string, value: string | number | undefined): void => {
      if (value === undefined) return
      if (!/^\S+$/.test(String(value))) throw new Error(`a /hud_login ${name} takes no whitespace: ${JSON.stringify(String(value))}`)
      flags.push(`${name} ${value}`)
    }
    flag('--name', options.name)
    flag('--look', options.look)
    if (options.guest) flags.push('--guest')
    if (options.defaultOnError) flags.push('--default-on-error')
    flag('--realm', options.realm)
    flag('--position', options.position)
    return this.json<HudLoginReply>(['/hud_login', payload, ...flags].join(' '))
  }

  /** A session-only guest made in the engine (no network). Takes effect the next frame; it is
   *  ignored once signed in with the world not held, so the caller checks `/world_state`. */
  async loginGuest(): Promise<void> {
    await this.run('/login_guest')
    await until(() => this.worldState(), (state) => state.signedIn, { timeoutMs: 30_000, intervalMs: 250, what: 'the guest to be signed in' })
  }

  /** The engine's sign-out (`/logout`). On the web the HUD never hears of it and stays signed in,
   *  refusing a new `/hud_login`; `/hud_logout` signs both out. */
  async logout(): Promise<void> {
    await this.run('/logout')
  }

  /** `/get_user_data`, parsed from `<name> (<address>): v<version>, web3=<bool>`. */
  async userData(address?: string): Promise<UserData> {
    const reply = await this.run(address ? `/get_user_data ${address}` : '/get_user_data')
    const match = /^(.*) \((0x[0-9a-fA-F]{40})\): v(\d+), web3=(true|false)$/s.exec(reply.trim())
    if (!match) throw new Error(`unparseable get_user_data: ${reply}`)
    const data = { name: match[1]!, address: match[2]!, version: Number(match[3]), web3: match[4] === 'true' }
    if (address === undefined) {
      // Each edit is one version above the last (`bump_version`), so a read that skipped some counts
      // them all; a version a failed deploy used and the next edit skips counts too, which only
      // waits longer.
      if (this.#ownVersion !== undefined && data.version !== this.#ownVersion) this.#deploymentSeen(Math.max(1, data.version - this.#ownVersion))
      this.#ownVersion = data.version
    }
    return data
  }

  /** Count a deployment of the own profile from now that no version change showed: the sign-in,
   *  whose first login deploys a new profile before any version was read. */
  profileDeployed(): void {
    this.#deploymentSeen()
  }

  #deploymentSeen(count = 1): void {
    this.#ownVersionChangedAt = Date.now()
    for (let i = 0; i < count; i++) this.#deploymentsSeenAt.push(this.#ownVersionChangedAt)
  }

  /** Before a step that deploys the own profile (closing the Backpack on a changed look, saving
   *  the passport): wait out the catalyst's window since the profile was last seen to change, a
   *  deployment of its own, and, with a burst's worth of deployments in its span, until the oldest
   *  of them leaves it. A change since the last read counts from now. */
  async profileDeployWindow(): Promise<void> {
    await this.userData()
    const left = this.#ownVersionChangedAt + this.#deployWindowMs - Date.now()
    if (left > 0) await sleep(left)
    const { count, spanMs } = this.#deployBurst
    const recent = this.#deploymentsSeenAt
    while (recent.length > 0 && recent[0]! <= Date.now() - spanMs) recent.shift()
    if (recent.length >= count) {
      const wait = recent[recent.length - count]! + spanMs - Date.now()
      if (wait > 0) await sleep(wait)
    }
  }

  /** The profile version, which bumps on every avatar change (an equip is observable here). */
  async profileVersion(): Promise<number> {
    return (await this.userData()).version
  }

  // --- avatar and chat ----------------------------------------------------------------------

  avatarState(which?: 'all' | string): Promise<AvatarReport[]> {
    return this.json<AvatarReport[]>(which ? `/avatar_state ${which}` : '/avatar_state')
  }

  /** The local player's avatar report. */
  async myAvatar(): Promise<AvatarReport> {
    const reports = await this.avatarState()
    const player = reports.find((report) => report.kind === 'player')
    if (!player) throw new Error('no player avatar in /avatar_state')
    return player
  }

  /** Play an emote by urn, built-in name or profile slot number. Only queues the request. */
  async emote(urnOrSlot: string | number, options: { loop?: boolean; upperBody?: boolean } = {}): Promise<void> {
    const flags = [options.loop ? '--loop' : '', options.upperBody ? '--upper-body' : ''].filter(Boolean)
    await this.run(['/emote', String(urnOrSlot), ...flags].join(' '))
  }

  chatHistory(count = 50): Promise<ChatHistory> {
    return this.json<ChatHistory>(`/chat_history ${count}`)
  }

  /** A direct message; `/dm` takes the rest of the line, so runs of spaces collapse to one. Not a
   *  delivery confirmation. */
  async dm(address: string, message: string): Promise<void> {
    await this.run(`/dm ${address} ${message}`)
  }

  /** The stored lines of a conversation: a partner's wallet, or a community's `community:<id>`. */
  async dmHistory(address: string, count = 20): Promise<string[]> {
    const reply = await this.run(`/dm_history ${address} ${count}`)
    return reply === '' ? [] : reply.split('\n')
  }

  /** A community chat message; `/community_chat` takes several words. Not a delivery confirmation. */
  async communityChat(communityId: string, message: string): Promise<void> {
    await this.run(`/community_chat ${communityId} ${message}`)
  }

  /** The community channels the engine listens to, after an optional edit. */
  async communityChannels(edit?: { action: 'add' | 'remove'; communityId: string } | { action: 'clear' }): Promise<string[]> {
    const args = edit == null ? '' : edit.action === 'clear' ? ' clear' : ` ${edit.action} ${edit.communityId}`
    const reply = await this.run(`/community_channels${args}`)
    const listed = /community channels: \[(.*)\]$/.exec(reply)?.[1]
    if (listed === undefined) throw new Error(`unexpected /community_channels reply: ${reply}`)
    return listed === '' ? [] : listed.split(',').map((id) => id.trim().replace(/^"|"$/g, ''))
  }

  communityState(): Promise<CommunityState> {
    return this.json<CommunityState>('/community_state')
  }

  /** Other players' addresses, then the local player's. */
  async connectedPlayers(): Promise<string[]> {
    const reply = await this.run('/connected_players')
    return reply === 'no other players connected' ? [] : reply.split(',').map((address) => address.trim())
  }

  voiceState(): Promise<VoiceState> {
    return this.json<VoiceState>('/voice_state')
  }

  nametags(): Promise<NametagState> {
    return this.json<NametagState>('/nametag_state')
  }

  /** Shows `message` in a chat bubble over the named nametags, on this client only; how many
   *  bubbles were asked for. */
  async nametagBubble(target: 'local' | 'all' | string, kind: 'nearby' | 'community' | 'dm', message: string): Promise<number> {
    const reply = await this.json<{ bubbles: number }>(`/nametag_bubble ${target} ${kind} ${message}`)
    return reply.bubbles
  }

  connectivity(): Promise<ConnectivityState> {
    return this.json<ConnectivityState>('/connectivity_state')
  }

  retryConnectivity(): Promise<string> {
    return this.run('/connectivity_retry')
  }

  /** The ban and block status; with an address, also that address's standing. */
  banBlockState(address?: string): Promise<BanBlockState> {
    return this.json<BanBlockState>(address ? `/ban_block_state ${address}` : '/ban_block_state')
  }

  /** Block a user through the social service; answers once the service has. */
  async blockUser(address: string): Promise<void> {
    await this.run(`/block_user ${address}`)
  }

  /** Lift a block through the social service; answers once the service has. */
  async unblockUser(address: string): Promise<void> {
    await this.run(`/unblock_user ${address}`)
  }

  /** A voice chat action, as the HUD's buttons do it (`/voice`, through the HUD's own path and
   *  refusals). Only queues the request: `/voice_state` `call` follows. */
  async voice(...action: VoiceAction): Promise<void> {
    await this.run(['/voice', ...action].join(' '))
  }

  nearbyMutes(): Promise<NearbyMutes> {
    return this.json<NearbyMutes>('/nearby_mutes')
  }

  /** Mute or unmute a wallet's nearby voice, as the profile menu's Hush and Hear do. */
  async nearbyMute(address: string, muted: boolean): Promise<void> {
    await this.run(`/nearby_mute ${muted ? 'on' : 'off'} ${address}`)
  }

  /** The sky time: `/time` reports; with `hours` (and `speed`, 0 stops the clock) it sets. */
  async time(hours?: number, speed?: number): Promise<{ before: string; after: string; speed: number }> {
    const reply = await this.run(['/time', hours === undefined ? '' : String(hours), speed === undefined ? '' : String(speed)].join(' '))
    const match = /^time (\d+:\d+) -> (\d+:\d+), speed ([\d.-]+)/.exec(reply)
    if (!match) throw new Error(`unparseable time: ${reply}`)
    return { before: match[1]!, after: match[2]!, speed: Number(match[3]) }
  }

  /** The engine's frame as PNG bytes at full size (the canvas on the web, the HUD included
   *  natively); with options, the scaled image and its caption, and with an empty options object
   *  the image at most 1920 px wide (see `ScreenshotOptions`). Refused while another capture runs. */
  async screenshot(): Promise<Buffer>
  async screenshot(options: ScreenshotOptions): Promise<CapturedShot>
  async screenshot(options?: ScreenshotOptions): Promise<Buffer | CapturedShot> {
    if (!options) return Buffer.from(await this.run('/screenshot'), 'base64')
    const flags = captureFlags(options)
    const shot = await this.json<Omit<CapturedShot, 'image'> & { image: string }>(`/screenshot${flags === '' ? ' --max-width 1920' : flags}`)
    return { ...shot, image: Buffer.from(shot.image, 'base64') }
  }

  /** `count` frames `intervalSeconds` apart, as paths; with `saveTo`, frames the engine kept in
   *  memory (the web client) are written there too. */
  async captureSequence(options: CaptureSequenceOptions = {}): Promise<CaptureSequence> {
    const flags = ` --count ${options.count ?? 5}${options.intervalSeconds === undefined ? '' : ` --interval-seconds ${options.intervalSeconds}`}${captureFlags(options)}${options.dir ? ` --dir ${quoteWord(options.dir)}` : ''}`
    const sequence = await this.json<CaptureSequence>(`/screenshot${flags}`)
    return options.saveTo ? saveCaptureFrames(this, sequence, options.saveTo) : sequence
  }

  /** The capture running now, if any, and what the last in-memory sequence keeps. */
  async captureState(): Promise<CaptureState> {
    return this.json<CaptureState>('/screenshot --state')
  }

  /** One frame of the last in-memory sequence, by its `path`. */
  async captureFrame(framePath: string): Promise<Buffer> {
    return Buffer.from(await this.run(`/screenshot --frame ${quoteWord(framePath)}`), 'base64')
  }

  /** Lay the HUD out at `[width, height]` CSS px, or at the window's size again with `null`, and
   *  resolve once it has drawn at that size. Throws on a transport without `resizeView`. */
  async resizeView(size: [number, number] | null): Promise<void> {
    if (!this.console.resizeView) throw new Error(`the ${this.console.kind} transport cannot resize the view`)
    await this.console.resizeView(size)
  }

  // --- input --------------------------------------------------------------------------------

  /** A key by its `KeyboardEvent.code`; `state` holds or releases it. */
  async key(code: string, state?: 'down' | 'up'): Promise<void> {
    await this.run(state ? `/press_key ${code} --state ${state}` : `/press_key ${code}`)
  }

  escape(): Promise<void> {
    return this.key('Escape')
  }

  /** After a HUD text field took the focus (a fill): wait for the engine to hand it the keyboard, as
   *  `/world_state` reports it; until then a key is still the engine's (Enter opens the chat,
   *  Escape closes the page) and not typed. A client that does not report it is not waited on. */
  async keyboardTaken(timeoutMs = KEYBOARD_TIMEOUT_MS): Promise<void> {
    await this.#keyboard((claimed) => claimed !== false, timeoutMs, 'the engine to hand the keyboard to the HUD field')
  }

  /** After a HUD text field lost the focus (an Escape out of it, a click that closed it): wait for
   *  the engine to let the keyboard go, as `/world_state` reports it; a key sent sooner is typed
   *  into nothing. A client that does not report it (an older build) is not waited on. */
  async keyboardReleased(timeoutMs = KEYBOARD_TIMEOUT_MS): Promise<void> {
    await this.#keyboard((claimed) => claimed !== true, timeoutMs, 'the engine to let the keyboard go (a HUD field had it)')
  }

  /** Wait for the engine's word on the keyboard to be `done`. Null (a client that takes no input)
   *  is its own answer; a client whose `/world_state` has no such word is older than the suite and
   *  is refused, not raced. */
  async #keyboard(done: (claimed: boolean | null) => boolean, timeoutMs: number, what: string): Promise<void> {
    const first = await this.worldState()
    if (first.keyboardClaimed === undefined) throw new Error(OLDER_CLIENT)
    if (done(first.keyboardClaimed)) return
    await until(() => this.worldState(), (state) => done(state.keyboardClaimed ?? null), { timeoutMs, intervalMs: 50, what })
  }

  /** A click at window pixels (as `/entity_screen_position` and `/hud_read` report them), or, with
   *  `normalized`, at 0 to 1 of the window from the top left. */
  async clickAt(x: number, y: number, button: MouseButton = 'left', options: { normalized?: boolean } = {}): Promise<ClickAtReply> {
    const flags = `${button === 'left' ? '' : ` --button ${button}`}${options.normalized ? ' --normalized' : ''}`
    return this.json<ClickAtReply>(`/click_at ${x} ${y}${flags}`)
  }

  // --- scenes -------------------------------------------------------------------------------

  /** The target scene's live entities as `/scene_entities` lists them (`root(0)`, `512`, ...). */
  async sceneEntities(component?: string): Promise<string[]> {
    const reply = await this.run(component ? `/scene_entities ${component}` : '/scene_entities')
    return reply === '(no entities)' ? [] : reply.split('\n')
  }

  /** Point the scene commands (`/scene_entities`, `/inspect_component`, `/scene_logs`) at a scene
   *  by hash or title; without one they follow the player's parcel again. */
  async setScene(hashOrTitle?: string): Promise<void> {
    await this.run(hashOrTitle ? `/set_scene ${hashOrTitle}` : '/set_scene')
  }

  /** The `value` of every `UiText` in the target scene: what its screen-space UI shows. */
  async uiTexts(): Promise<string[]> {
    const entities = await this.sceneEntities('UiText')
    const texts: string[] = []
    for (const entity of entities) {
      const id = /\((\d+)\)$/.exec(entity)?.[1] ?? entity
      const text = await this.inspect<{ value?: string }>(id, 'UiText')
      if (typeof text.value === 'string') texts.push(text.value)
    }
    return texts
  }

  /** The scenes running: parcel scenes and portable experiences. */
  async liveScenes(): Promise<LiveScene[]> {
    return parseLiveScenes(await this.run('/live_scenes'))
  }

  async smartWearables(): Promise<SmartWearables> {
    return parseSmartWearables(await this.run('/smart_wearables'))
  }

  /** A smart wearable's entry, or undefined when the engine does not track it. */
  async smartWearable(urn: string): Promise<SmartWearableEntry | undefined> {
    return (await this.smartWearables()).entries.find((entry) => entry.urn === urn)
  }

  /** The portable experiences the HUD's rail lists. */
  async portables(): Promise<Portable[]> {
    return parsePortables(await this.run('/portables'))
  }

  /** `/stop_portable <hash>`: stop a listed portable as the HUD's `Stop` does; a reply other than
   *  `stopped <hash>` is an error. */
  async stopPortable(hash: string): Promise<void> {
    const reply = (await this.run(`/stop_portable ${hash}`)).trim()
    if (reply !== `stopped ${hash}`) throw new Error(`/stop_portable ${hash} replied: ${reply}`)
  }

  /** `/smart_wearable run|kill|restart <urn>`; a reply other than `<running|killed|restarted>
   *  <urn>` is an error. */
  async smartWearableAction(action: SmartWearableAction, urn: string): Promise<void> {
    const reply = (await this.run(`/smart_wearable ${action} ${urn}`)).trim()
    if (reply !== `${SMART_WEARABLE_REPLIES[action]} ${urn}`) throw new Error(`/smart_wearable ${action} ${urn} replied: ${reply}`)
  }

  async inspect<T = unknown>(entity: string | number, component: string): Promise<T> {
    return this.json<T>(`/inspect_component ${entity} ${component}`)
  }

  entityScreenPosition(entity: string | number, options: { bounds?: boolean } = {}): Promise<ScreenPosition> {
    return this.json<ScreenPosition>(`/entity_screen_position ${entity}${options.bounds ? ' --bounds' : ''}`)
  }

  /** Aims the pointer at an entity and reports what it hovers; a blocked outcome resolves with `diagnosis` set; a refusal rejects. */
  hoverEntity(entity: string | number, options: { bounds?: boolean; frames?: number; timeoutFrames?: number } = {}): Promise<PointerReport> {
    let line = `/hover_entity ${entity}`
    if (options.bounds) line += ' --bounds'
    if (options.frames !== undefined) line += ` --frames ${options.frames}`
    if (options.timeoutFrames !== undefined) line += ` --timeout-frames ${options.timeoutFrames}`
    return this.json<PointerReport>(line)
  }

  async clearPointer(): Promise<void> {
    await this.run('/hover_entity --clear')
  }

  clickEntity(
    entity: string | number,
    options: { bounds?: boolean; button?: 'pointer' | 'primary' | 'secondary'; event?: 'click' | 'down' | 'up'; timeoutFrames?: number } = {}
  ): Promise<PointerReport> {
    let line = `/click_entity ${entity}`
    if (options.bounds) line += ' --bounds'
    if (options.button) line += ` --button ${options.button}`
    if (options.event) line += ` --event ${options.event}`
    if (options.timeoutFrames !== undefined) line += ` --timeout-frames ${options.timeoutFrames}`
    return this.json<PointerReport>(line)
  }

  raycastStats(): Promise<RaycastStats> {
    return this.json<RaycastStats>('/raycast_stats')
  }

  /** The capped raycast tier's budget in ms; with `ms`, overrides it first, and `'auto'` returns it to a tenth of the target frame. */
  async raycastBudget(ms?: number | 'auto'): Promise<number> {
    const reply = await this.run(ms === undefined ? '/raycast_budget' : `/raycast_budget ${ms}`)
    const match = /^raycast budget: ([0-9.]+) ms \((auto|override)\)$/.exec(reply.trim())
    if (!match) throw new Error(`unexpected /raycast_budget reply: ${reply}`)
    return Number(match[1])
  }

  mediaState(): Promise<MediaState> {
    return this.json<MediaState>('/media_state')
  }

  /** The wait before a failed web media source's first reopen; with `ms`, sets it first. */
  mediaRetryDelay(ms?: number): Promise<MediaRetryDelay> {
    return this.json<MediaRetryDelay>(ms === undefined ? '/media_retry_delay' : `/media_retry_delay ${ms}`)
  }

  /** The target scene's last log lines, oldest first; a leading "(+N older entries not retained)" line is kept. */
  async sceneLogs(count = 20): Promise<string[]> {
    const reply = await this.run(`/scene_logs ${count}`)
    return reply === '(no logs)' ? [] : reply.split('\n')
  }

  async close(): Promise<void> {
    await this.console.close()
  }
}

/** The HUD by role and accessible name, through `/hud_*`. */
export class Hud {
  readonly #explorer: Explorer
  readonly #allowSynthetic: boolean

  constructor(explorer: Explorer, allowSynthetic: boolean) {
    this.#explorer = explorer
    this.#allowSynthetic = allowSynthetic
  }

  /** The visible controls with a name, in document order, filtered by role. */
  list(role?: string): Promise<HudEntry[]> {
    return this.#explorer.json<HudEntry[]>(role ? `/hud_list ${role}` : '/hud_list')
  }

  /** A control's description; `found: false` when none (or several without an index). */
  read(selector: Selector): Promise<HudRead> {
    return this.#explorer.json<HudRead>(`/hud_read ${selectorArg(selector)}`)
  }

  /** The control, or undefined when it is not visible. Ambiguity is an error. */
  async find(selector: Selector): Promise<Control | undefined> {
    const read = await this.read(selector)
    if (read.found) return read
    if (read.error) throw new Error(`${describeSelector(selector)}: ${read.error}`)
    return undefined
  }

  async isVisible(selector: Selector): Promise<boolean> {
    return (await this.find(selector)) !== undefined
  }

  /** The control's `text`; an error when it is not visible. */
  async text(selector: Selector): Promise<string> {
    return (await this.get(selector)).text
  }

  /** The control's `value` (a field); an error when it is not visible or has none. */
  async value(selector: Selector): Promise<string> {
    const control = await this.get(selector)
    if (control.value === undefined) throw new Error(`${describeSelector(selector)} has no value: it is a ${control.role}`)
    return control.value
  }

  /** The control; an error naming it when it is not visible. */
  async get(selector: Selector): Promise<Control> {
    const control = await this.find(selector)
    if (!control) throw new Error(`no visible HUD control matches ${describeSelector(selector)}`)
    return control
  }

  /** A real click on the control's centre, once a click would land there (`actionable`). Fails
   *  on a synthetic (DOM-event) click unless the explorer allows it. */
  async click(selector: Selector): Promise<HudInput> {
    await this.waitActionable(selector)
    const result = await this.#explorer.json<HudInput>(`/hud_click ${selectorArg(selector)}`)
    this.#checkInput(result, `click on ${describeSelector(selector)}`)
    return result
  }

  /** Replace a field's text, once a click would land on it; the reply holds the field as the
   *  typing left it. A real fill leaves the field focused: it returns once the engine has handed
   *  the field the keyboard, so the next key is the field's. */
  async fill(selector: Selector, text: string): Promise<HudInput> {
    await this.waitActionable(selector)
    const result = await this.#explorer.json<HudInput>(`/hud_fill ${JSON.stringify({ ...toSelectorObject(selector), text })}`)
    this.#checkInput(result, `fill of ${describeSelector(selector)}`)
    if (result.input === 'trusted') await this.#explorer.keyboardTaken()
    return result
  }

  /** Wait for a control to be `visible` (default), `hidden`, `enabled` or `actionable`. */
  wait(selector: Selector, options: { state?: HudState; timeoutMs?: number } = {}): Promise<HudRead> {
    const { state = 'visible', timeoutMs = DEFAULT_HUD_TIMEOUT_MS } = options
    return this.#explorer.json<HudRead>(`/hud_wait ${JSON.stringify({ ...toSelectorObject(selector), state, timeout: timeoutMs })}`)
  }

  waitHidden(selector: Selector, timeoutMs = DEFAULT_HUD_TIMEOUT_MS): Promise<HudRead> {
    return this.wait(selector, { state: 'hidden', timeoutMs })
  }

  /** The control once a click would land on it: visible, enabled, in the viewport, uncovered and
   *  no longer moving (the HUD reports a control on its way in, or growing in, as still moving: a
   *  click aimed at it there lands where it no longer is). */
  async waitActionable(selector: Selector, timeoutMs = ACTIONABLE_TIMEOUT_MS): Promise<Control> {
    const read = await this.wait(selector, { state: 'actionable', timeoutMs })
    if (!read.found) throw new Error(`no visible HUD control matches ${describeSelector(selector)}`)
    return read
  }

  /** The control once nothing around it is still loading (`ready`: no `aria-busy` region holds
   *  it), such as a grid whose newest page has arrived. */
  async waitReady(selector: Selector, timeoutMs = DEFAULT_HUD_TIMEOUT_MS): Promise<Control> {
    const read = await this.wait(selector, { state: 'ready', timeoutMs })
    if (!read.found) throw new Error(`no visible HUD control matches ${describeSelector(selector)}`)
    return read
  }

  /** Put the pointer on a control, for the controls that appear on hover: a real click at its
   *  centre once a click would land there, which the browser precedes with a move. It also
   *  selects the control, as a user's click would. */
  async hover(selector: Selector): Promise<Control> {
    const control = await this.waitActionable(selector)
    await this.#explorer.clickAt(control.x + control.width / 2, control.y + control.height / 2)
    return control
  }

  #checkInput(result: HudInput, what: string): void {
    if (result.input !== 'trusted' && !this.#allowSynthetic) {
      throw new ConsoleError(what, `the ${what} was synthetic (DOM events): run under the web driver, or allow synthetic input`)
    }
  }
}
