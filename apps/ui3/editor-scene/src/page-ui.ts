import { engine } from '@dcl/sdk/ecs'
import { BevyApi } from './bevy-api'
import { cmd } from './cmd'
import { log } from './log'
import { state, setActiveAction, topLevelSelected, setSelected, type Snapshot } from './state'
import { entityName, NAME_COMPONENT, stringToBase64 } from './custom-components'
import { reloadSnapshot, ensureProjectScenePinned } from './inspector/session'
import { applyExternalComponentWrite, writeDelete, flushTransforms } from './inspector/mutate'
import { setComponentValue, addComponent, deleteComponent } from './inspector/component-ops'
import { placeEntity } from './inspector/entities'
import { exportComposite } from './inspector/save'
import { importComposite, restoreComposite } from './inspector/hydrate'
import {
  setCamMode,
  orientToAxis,
  focusOrbitOn,
  focusSatellite,
  frameEntityOnce,
  adjustFlySpeed,
  cameraDropLocal,
  applyCameraInput,
  setCameraSettings,
  currentOrbitDistance,
  cameraPose,
  setPlayingCamera
} from './camera/free-cam'
import { endGizmoDrag } from './viewport/gizmo'
import { AUTHORING_RPCS, AUTHORING_MESSAGES, createAuthoringCheckpoint } from './authoring-checkpoint'
import { prepareDragTransition } from './drag-notification'
import { moveHierarchy, copyEntities, pasteEntities, copyComponent, pasteComponent, writeComponents, writeMaterials, removeComponent, removeEntities, addEntity as addEntityRpc, addComponent as addComponentRpc, orderHierarchyEntities } from './hierarchy-runtime'
import { createRuntimeReset } from './runtime-reset'
import { createRuntimeMonitor, replacementProjectScene } from './runtime-monitor'
import { createEditorScheduler, runEditorTick } from './page-ui-scheduler'
import { runRpcOperation } from './rpc-operation'
import { applyPendingGltfLoadingState, selectedPendingGltf, type PendingGltf } from './pending-gltf-refresh'
import { createProjectRead } from './project-read'
import { createPlaybackController } from './playback-controller'
import { setPlayMode } from './edit-freeze'
import { sleep } from './utils'
import {
  type BusEnvelope,
  type PageToSceneMessage,
  type SceneToPageMessage,
  type EditorTool,
  type EditorEntityNode,
  EDITOR_BUS_CHANNEL,
  SCENE_BRIDGE_VERSION
} from './bridge-protocol'

declare const BroadcastChannel: { new (name: string): {
  postMessage(msg: unknown): void
  close(): void
  onmessage: ((ev: { data: unknown }) => void) | null
} }

let previewContents: { hash: string; files: Record<string, string> } | null = null
async function restorePreviewContents(): Promise<void> {
  if (!previewContents || previewContents.hash !== state.scene?.hash) return
  const files = previewContents.files
  const count = await cmd.registerContent(stringToBase64(JSON.stringify(files)))
  if (count !== Object.keys(files).length) throw new Error('The renderer did not restore all project assets')
}

const POLL_INTERVAL_S = 0.1
const RUNTIME_MONITOR_INTERVAL_MS = 1000
const monitorRuntime = createRuntimeMonitor({
  stats: () => cmd.sceneStats(),
  needsRecovery: () => state.status === 'error',
  changed: async () => {
    if (state.frozen) {
      try { await cmd.freezeScene() }
      catch (error) { if (!String(error).includes('already frozen')) throw error }
    }
    await reloadSnapshot()
    if (state.status !== 'ready') throw new Error(state.error || 'The updated scene could not be read')
    await restorePreviewContents()
    readyAnnounced = false
  }
})
const RPC_TIMEOUT_MS = 12000
const readProject = createProjectRead({
  stats: () => cmd.sceneStats(),
  refresh: async () => {
    await flushTransforms()
    await reloadSnapshot()
    if (state.status !== 'ready') throw new Error(state.error || 'The project snapshot could not be read')
  },
  sleep,
  now: () => Date.now()
})

const checkpointAuthoring = createAuthoringCheckpoint({
  project: () => state.scene ? { hash: state.scene.hash, playing: state.playing } : null,
  stats: () => cmd.sceneStats(),
  flush: flushTransforms,
  read: async () => {
    await reloadSnapshot()
    if (state.status !== 'ready') throw new Error(state.error || 'The authored scene could not be read')
    return exportComposite()
  },
  send: checkpoint => send({ type: 'authoring-checkpoint', ...checkpoint })
})

let channel: InstanceType<typeof BroadcastChannel> | null = null

// The engine's sandbox suffixes a super-user scene's channel names with the session the hosting
// page seeded (window.__bridgeSession, the editor's session), so the bare name reaches only the
// editor that launched this viewport.
function connectEditorBus(): void {
  const next = new BroadcastChannel(EDITOR_BUS_CHANNEL)
  next.onmessage = (ev): void => {
    const env = ev.data as BusEnvelope | null
    if (env === null || typeof env !== 'object') return
    if (env.to === 'scene') scheduler.enqueue(env.msg)
  }
  channel?.close()
  channel = next
}

let editorProjection: 'ortho' | 'perspective' = 'perspective'
const setPlayback = createPlaybackController({
  pin: async () => state.status === 'ready' && state.scene !== undefined &&
    await ensureProjectScenePinned(state.scene.hash),
  freeze: () => cmd.freezeScene(),
  unfreeze: () => cmd.unfreezeScene(),
  visuals: async (playing, paused) => {
    if (playing !== state.playing) {
      await cmd.setCameraProjection(playing ? 'perspective' : editorProjection, currentOrbitDistance())
    }
    await setPlayMode(playing, paused)
  },
  commit: (playing, paused) => {
    if (state.gizmoDragging) endGizmoDrag()
    state.selectBox = null
    state.gizmoHover = null
    state.playing = playing
    state.frozen = paused
    setPlayingCamera(playing)
  }
})

let resetting = false
const resetRuntime = createRuntimeReset({
  stats: () => cmd.sceneStats(),
  pin: (hash) => cmd.setScene(hash),
  reload: (hash) => cmd.reload(hash),
  freeze: () => cmd.freezeScene(),
  restore: async (composite) => {
    state.frozen = true
    await reloadSnapshot()
    if (state.status !== 'ready') throw new Error(state.error)
    await restorePreviewContents()
    await restoreComposite(composite)
  },
  finish: async () => {
    await cmd.setCameraProjection(editorProjection, currentOrbitDistance())
    await setPlayMode(false)
    state.playing = false
    state.frozen = true
    setPlayingCamera(false)
    await reloadSnapshot()
    if (state.status !== 'ready') throw new Error(state.error)
    monitorRuntime.accept(await cmd.sceneStats())
  },
  sleep,
  now: () => Date.now()
})

const RPC_METHODS = new Set([
  'getPreviousLogin',
  'loginPrevious',
  'loginGuest',
  'liveSceneInfo',
  // the engine console, for the editor page that launched this viewport (its own bus): the
  // Explorer's app runs in a document isolated from the page, which cannot call it directly
  'consoleCommand'
])

const LOCAL_RPC: Record<string, (...args: unknown[]) => unknown> = {
  moveHierarchy,
  writeComponents,
  writeMaterials,
  removeComponent,
  removeEntities,
  addEntity: addEntityRpc,
  addComponent: addComponentRpc,
  copyEntities,
  pasteEntities,
  copyComponent,
  pasteComponent,
  stopPlayback: async (composite) => {
    if (resetting) throw new Error('A scene reset is already in progress')
    if (typeof composite !== 'string') throw new Error('A scene composite is required')
    const hash = state.scene?.hash
    if (!hash || state.status !== 'ready') throw new Error('The scene is not ready for editing')
    resetting = true
    scheduler.invalidateMonitor()
    try {
      await resetRuntime(hash, composite)
    } finally {
      resetting = false
    }
  },
  stepPlayback: async (count) => {
    if (!state.playing || !state.frozen) throw new Error('Pause the preview before stepping')
    if (!state.scene || !await ensureProjectScenePinned(state.scene.hash)) {
      throw new Error('The project scene could not be selected')
    }
    if (typeof count !== 'number' || !Number.isSafeInteger(count) || count < 1) {
      throw new Error('A positive tick count is required')
    }
    return cmd.tickScene(count)
  },
  setPlayback: (playing, paused) => {
    if (resetting) throw new Error('Wait for the scene reset to finish')
    if (typeof playing !== 'boolean' || typeof paused !== 'boolean') {
      throw new Error('Playback requires playing and paused flags')
    }
    if (!playing) throw new Error('Use stopPlayback with the authored snapshot to reset the scene')
    return setPlayback(playing, paused)
  },
  cameraDrop: () => cameraDropLocal(),
  sceneStats: async () => {
    const hash = state.scene?.hash
    if (hash !== undefined) await ensureProjectScenePinned(hash)
    return cmd.sceneStats()
  },
  diag: async (count: unknown) => {
    const hash = state.scene?.hash
    if (hash !== undefined) await ensureProjectScenePinned(hash)
    const stats = await cmd.sceneStats().catch((e) => `stats-err:${e}`)
    const logs = await cmd.sceneLogs(typeof count === 'number' ? count : 25).catch((e) => `logs-err:${e}`)
    return { stats, logs }
  },
  exportComposite: () => {
    const hash = state.scene?.hash
    if (!hash) throw new Error('The scene is not ready for export')
    return readProject(hash, exportComposite)
  },
  restoreComposite: async (composite: unknown) => {
    if (typeof composite !== 'string') throw new Error('A scene composite is required')
    if (state.status !== 'ready') throw new Error('The scene is not ready for editing')
    if (!state.scene || !await ensureProjectScenePinned(state.scene.hash)) {
      throw new Error('The project scene could not be selected')
    }
    scheduler.invalidateMonitor()
    await restorePreviewContents()
    await restoreComposite(composite)
  },
  registerContent: async (contents: unknown) => {
    if (!state.scene || !await ensureProjectScenePinned(state.scene.hash)) {
      throw new Error('The project scene could not be selected')
    }
    const hash = state.scene.hash
    const count = await cmd.registerContent(stringToBase64(JSON.stringify(contents)))
    const files = contents as Record<string, string>
    if (count !== Object.keys(files).length) throw new Error('The renderer did not register every project asset')
    previewContents = { hash, files: { ...(previewContents?.hash === hash ? previewContents.files : {}), ...files } }
    return count
  },
  initAsset: async (assetId: unknown, contents: unknown) => {
    const hash = state.scene?.hash
    if (hash !== undefined) await ensureProjectScenePinned(hash)
    return cmd.initAsset(String(assetId), stringToBase64(JSON.stringify(contents ?? {})))
  }
}

let readyAnnounced = false
let lastSelectionSig = ''
let lastTool = ''
let lastDragging = false
let lastEntitiesSig = ''

const RESERVED_ENTITIES = new Set(['0', '1', '2'])

function buildEntityList(snapshot: Snapshot): EditorEntityNode[] {
  const out: EditorEntityNode[] = []
  for (const id of Object.keys(snapshot)) {
    if (RESERVED_ENTITIES.has(id)) continue
    const comps = snapshot[id]
    if (comps?.[NAME_COMPONENT] === undefined) continue
    const transform = comps.Transform as { parent?: number } | undefined
    const parent = transform?.parent === undefined ? '0' : String(transform.parent)
    out.push({ id, name: entityName(snapshot, id) ?? null, parent })
  }
  return orderHierarchyEntities(out)
}

function entitiesSig(list: EditorEntityNode[]): string {
  return list.map((e) => `${e.id}:${e.name ?? ''}:${e.parent}`).join('|')
}

let lastSnapshotRef: Snapshot | null = null
let cachedList: EditorEntityNode[] = []
function currentEntityList(): EditorEntityNode[] {
  if (state.snapshot !== lastSnapshotRef) {
    lastSnapshotRef = state.snapshot
    cachedList = buildEntityList(state.snapshot)
  }
  return cachedList
}

export function startPageUiBridge(): void {
  connectEditorBus()

  let elapsed = 0
  let busy = false
  let lastPose = ''
  engine.addSystem((dt: number) => {
    elapsed += dt
    if (elapsed < POLL_INTERVAL_S || busy) return
    elapsed = 0
    const pose = cameraPose()
    if (pose !== null) {
      const key = [pose.x, pose.y, pose.z, pose.yaw, pose.pitch].map((n) => n.toFixed(2)).join(',')
      if (key !== lastPose) {
        lastPose = key
        send({ type: 'camera-pose', ...pose })
      }
    }
    busy = true
    tick()
      .catch((e) => log.debug('bus tick failed', e))
      .then(() => {
        busy = false
      })
  })
}

interface RuntimeSample {
  hash: string
  stats: string
  loading?: { pending: PendingGltf; snapshot: Snapshot }
}

async function tick(): Promise<void> {
  await runEditorTick(scheduler.run, notifyChanges, sampleRuntime)
}

function sampleRuntime(): void {
  const scene = !resetting ? state.scene : undefined
  const pending = selectedPendingGltf(state.snapshot, state.selected, state.activeEntity)
  if (scene) scheduler.sample(Date.now(), async () => {
    const loading = pending === undefined ? Promise.resolve(undefined) : cmd.crdtSnapshot()
      .then((snapshot) => ({ pending, snapshot }))
      .catch((error) => { log.debug('Pending GLTF state refresh failed', error); return undefined })
    const [stats, sample] = await Promise.all([cmd.sceneStats().catch(() => ''), loading])
    return { hash: scene.hash, stats, loading: sample }
  })
}

const scheduler = createEditorScheduler<PageToSceneMessage, RuntimeSample>({
  handle: async (message) => {
    try {
      if (AUTHORING_MESSAGES.has(message.type)) await checkpointAuthoring(() => handle(message))
      else await handle(message)
    }
    catch (error) { console.error('page-ui: failed to handle message', message, error) }
  },
  recover: async ({ hash, stats, loading }) => {
    if (resetting || state.scene?.hash !== hash) return
    const replacement = replacementProjectScene(state.scene, await BevyApi.liveSceneInfo())
    if (!replacement) return
    if (replacement.hash !== hash || !stats) {
      await cmd.setScene(replacement.hash)
      stats = await cmd.sceneStats()
      state.scene = replacement
    }
    if (loading !== undefined) state.snapshot = applyPendingGltfLoadingState(state.snapshot, loading.pending, loading.snapshot)
    await monitorRuntime(replacement.hash, stats)
  },
  onMonitorError: (error) => log.debug('Project runtime refresh pending', error),
  intervalMs: RUNTIME_MONITOR_INTERVAL_MS
})

async function handle(msg: PageToSceneMessage): Promise<void> {
  const wide = msg as { type: string; entity?: unknown; name?: unknown }
  if (wide.type === 'delete-component') {
    if (typeof wide.entity === 'string' && typeof wide.name === 'string') {
      await deleteComponent(wide.entity, wide.name)
    }
    return
  }
  switch (msg.type) {
    case 'init':
      readyAnnounced = false
      break
    case 'set-tool':
      setActiveAction(msg.tool)
      break
    case 'set-flags':
      if (msg.orientGlobal !== undefined) state.orientGlobal = msg.orientGlobal
      if (msg.pivotEach !== undefined) state.pivotEach = msg.pivotEach
      if (msg.showLinks !== undefined) state.showLinks = msg.showLinks
      break
    case 'set-selection':
      setSelected(msg.selected)
      state.activeEntity = msg.active
      lastSelectionSig = ''
      break
    case 'set-camera':
      if (state.playing) break
      setCamMode(msg.mode === 'off' ? 'none' : msg.mode)
      if (msg.axis !== undefined) {
        const sign = msg.axis.startsWith('-') ? -1 : 1
        const axis = msg.axis.replace(/^[+-]/, '') as 'x' | 'y' | 'z'
        if (axis === 'x' || axis === 'y' || axis === 'z') orientToAxis(axis, sign)
      }
      break
    case 'focus':
      if (state.playing) break
      setSelected([msg.entity])
      state.activeEntity = msg.entity
      if (msg.satellite === true) focusSatellite(msg.entity)
      else if (msg.orbit === false) frameEntityOnce(msg.entity)
      else focusOrbitOn(msg.entity)
      break
    case 'refresh':
      if (!resetting) await reloadSnapshot()
      break
    case 'pointer-up':
      if (state.gizmoDragging) endGizmoDrag()
      break
    case 'fly-speed':
      adjustFlySpeed(msg.factor)
      break
    case 'camera-input':
      if (state.playing) break
      applyCameraInput(msg)
      break
    case 'camera-settings':
      setCameraSettings(msg.sensitivity, msg.invertY)
      break
    case 'set-camera-projection':
      if (state.playing) break
      editorProjection = (await cmd.setCameraProjection(msg.ortho, currentOrbitDistance()))
        .startsWith('orthographic') ? 'ortho' : 'perspective'
      break
    case 'resync':
      await reloadSnapshot()
      break
    case 'component-written':
      applyExternalComponentWrite(msg.entity, msg.name, msg.json)
      break
    case 'set-component':
      await setComponentValue(msg.entity, msg.name, msg.json)
      break
    case 'add-component':
      await addComponent(msg.entity, msg.name)
      break
    case 'load-scene':
      if (msg.replace === true) {
        scheduler.invalidateMonitor()
        await restoreComposite(msg.composite)
      } else {
        await importComposite(msg.composite)
      }
      break
    case 'add-entity':
      await placeEntity(
        msg.name,
        msg.parent,
        msg.components ?? null,
        typeof msg.dropX === 'number' && typeof msg.dropY === 'number'
          ? { x: msg.dropX, y: msg.dropY }
          : null
      )
      break
    case 'entity-deleted':
      if (RESERVED_ENTITIES.has(msg.entity)) break
      await writeDelete(msg.entity, msg.recursive)
      break
    case 'rpc':
      await handleRpc(msg)
      break
  }
}

async function handleRpc(msg: {
  id: number | string
  method: string
  args?: unknown[]
}): Promise<void> {
  const local = LOCAL_RPC[msg.method]
  if (local === undefined && !RPC_METHODS.has(msg.method)) {
    send({ type: 'rpc-reply', id: msg.id, ok: false, error: `unknown rpc ${msg.method}` })
    return
  }
  const fn = local ?? (BevyApi as unknown as Record<string, (...a: unknown[]) => unknown>)[msg.method]
  const timeoutMs = msg.method === 'stopPlayback' ? 40000 : RPC_TIMEOUT_MS
  await runRpcOperation(
    () => AUTHORING_RPCS.has(msg.method)
      ? checkpointAuthoring(() => fn(...(msg.args ?? [])))
      : fn(...(msg.args ?? [])),
    msg.method === 'writeMaterials' ? null : async () => {
      await sleep(timeoutMs)
      throw new Error(`rpc ${msg.method} timed out after ${timeoutMs}ms`)
    },
    outcome => send({ type: 'rpc-reply', id: msg.id, ...outcome })
  )
}

async function notifyChanges(): Promise<void> {
  if (!readyAnnounced && state.status === 'ready' && state.scene != null) {
    // The host releases its loading curtain after scene-ready. Select the edit
    // camera first so the player camera cannot appear during that handoff.
    if (!state.playing && state.camMode === 'none') setCamMode('target')
    readyAnnounced = true
    send({
      type: 'scene-ready',
      bridge: SCENE_BRIDGE_VERSION,
      scene: state.scene ?? null,
      frozen: state.frozen,
      tool: state.activeAction as EditorTool,
      orientGlobal: state.orientGlobal,
      pivotEach: state.pivotEach,
      selected: [...state.selected],
      active: state.activeEntity
    })
    lastSelectionSig = selectionSig()
    lastTool = state.activeAction
    lastDragging = state.gizmoDragging
    const list = currentEntityList()
    lastEntitiesSig = entitiesSig(list)
    send({ type: 'entities', entities: list })
    send({ type: 'selection', selected: [...state.selected], active: state.activeEntity, components: selectedComponents() })
    return
  }

  const entityList = currentEntityList()
  const entSig = entitiesSig(entityList)
  if (entSig !== lastEntitiesSig) {
    lastEntitiesSig = entSig
    send({ type: 'entities', entities: entityList })
  }

  const sig = selectionSig()
  if (sig !== lastSelectionSig) {
    lastSelectionSig = sig
    send({
      type: 'selection',
      selected: [...state.selected],
      active: state.activeEntity,
      components: selectedComponents()
    } as SceneToPageMessage)
  }
  if (state.activeAction !== lastTool) {
    lastTool = state.activeAction
    send({ type: 'tool', tool: state.activeAction as EditorTool })
  }
  const dragging = state.gizmoDragging
  if (dragging !== lastDragging) {
    const transforms: Record<string, unknown> = {}
    if (!dragging) {
      for (const id of topLevelSelected(state.snapshot)) {
        const t = state.snapshot[id]?.Transform
        if (t !== undefined) transforms[id] = t
      }
    }
    // Drain writes before acknowledging, but retain the observed drag/selection
    // if another gesture starts while the authoritative snapshot is exported.
    const transition = await prepareDragTransition(dragging, state.playing, transforms,
      () => checkpointAuthoring(async () => {}))
    lastDragging = transition.dragging
    send(transition.message)
  }
}

function selectionSig(): string {
  return JSON.stringify([[...state.selected].sort(), state.activeEntity, selectedComponents()])
}

function selectedComponents(): Record<string, Record<string, unknown>> {
  const out: Record<string, Record<string, unknown>> = {}
  for (const id of state.selected) {
    const comps = state.snapshot[id]
    if (comps !== undefined) out[id] = comps
  }
  return out
}

function send(msg: SceneToPageMessage): void {
  channel?.postMessage({ to: 'page', msg } satisfies BusEnvelope)
}
