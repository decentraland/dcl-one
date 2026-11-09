// dcl-scene-cli code-to-composite driver. Runs the scene's built bundle once
// under node against a mock renderer, keeps what the visual editor can edit
// and writes assets/scene/main.composite + main.crdt. Ported from upstream
// @dcl/sdk-commands commands/code-to-composite/{scene-executor,asset-migrator,
// name-generator,composite-generator}.ts; the bundle loading is the host
// runtime's (host-runtime.mjs), since the bundle is this CLI's split loader.
//
//   node code-to-composite.mjs <sceneRoot>
//
// The scene's @dcl/inspector-service is used (the vendored one, or a real
// install) with the ecs it exports: the editor engine is createEngineContext()'s,
// so the component set written is exactly what the editor reads back. The last
// stdout line is a JSON summary; scene console output goes to stderr.
'use strict'

import { createRequire } from 'node:module'
import fs from 'node:fs'
import path from 'node:path'

const root = path.resolve(process.argv[2] ?? '.')
const sceneJson = JSON.parse(fs.readFileSync(path.join(root, 'scene.json'), 'utf8'))
const mainFile = sceneJson.main
const req = createRequire(path.join(root, 'package.json'))

const inspector = req('@dcl/inspector-service')
const ECS = inspector.ecs
if (!ECS) {
  throw new Error(
    '@dcl/inspector-service does not export its ecs: run dcl-scene-cli init --node-modules-only to reinstall the vendored node_modules'
  )
}
const { ReadWriteByteBuffer } = ECS

// ---------------------------------------------------------------------------
// the editor engine: every component the inspector knows, plus react-ecs UI
// so UI entities can be recognised and dropped (upstream filter)
// ---------------------------------------------------------------------------
const { engine } = inspector.createEngineContext()
const UI_FACTORIES = [
  'UiTransform',
  'UiText',
  'UiBackground',
  'UiInput',
  'UiInputResult',
  'UiDropdown',
  'UiDropdownResult'
]
const uiComponentIds = new Set()
for (const name of UI_FACTORIES) {
  const factory = ECS.components[name]
  if (typeof factory === 'function') uiComponentIds.add(factory(engine).componentId)
}
const transport = { filter: () => true, send: async () => {} }
engine.addTransport(transport)

const Transform = engine.getComponent('core::Transform')
const Name = engine.getComponent('core-schema::Name')

async function feed(bytes) {
  transport.onmessage(bytes)
  await engine.update(0)
}

// ---------------------------------------------------------------------------
// the renderer's initial state for the scene (upstream getInitialCrdtState):
// a player and camera Transform, the UI canvas and the camera mode
// ---------------------------------------------------------------------------
function putOperation(entity, component, value) {
  const data = new ReadWriteByteBuffer()
  component.schema.serialize(value, data)
  const out = new ReadWriteByteBuffer()
  ECS.PutComponentOperation.write(entity, 1, component.componentId, data.toBinary(), out)
  return out.toBinary()
}

function initialCrdtState() {
  const transform = {
    position: { x: 0, y: 0, z: 0 },
    rotation: { x: 0, y: 0, z: 0, w: 1 },
    scale: { x: 1, y: 1, z: 1 },
    parent: 0
  }
  const state = [putOperation(1, Transform, transform), putOperation(2, Transform, transform)]
  const probe = ECS.Engine()
  for (const [entity, name] of [
    [0, 'UiCanvasInformation'],
    [2, 'CameraMode']
  ]) {
    const factory = ECS.components[name]
    if (typeof factory !== 'function') continue
    const component = factory(probe)
    state.push(putOperation(entity, component, component.create(probe.RootEntity)))
  }
  return state
}

const mainCrdtPath = path.join(root, 'main.crdt')
const existingCrdt = fs.existsSync(mainCrdtPath) ? new Uint8Array(fs.readFileSync(mainCrdtPath)) : null

// ---------------------------------------------------------------------------
// ~system/* for the bundle: Runtime serves the scene's own files (the loader
// reads its chunks through it), EngineApi pipes what the scene sends its
// renderer into the editor engine; everything else answers with empties
// ---------------------------------------------------------------------------
const STATIC = {
  '~system/Runtime': {
    readFile: async ({ fileName }) => ({
      content: new Uint8Array(fs.readFileSync(path.join(root, fileName))),
      hash: 'code-to-composite'
    }),
    getRealm: async () => ({
      realmInfo: {
        baseUrl: 'http://localhost',
        realmName: 'code-to-composite',
        networkId: 0,
        commsAdapter: 'offline:offline',
        isPreview: true
      }
    }),
    getWorldTime: async () => ({ seconds: 43200 }),
    getSceneInformation: async () => ({
      urn: 'urn:code-to-composite',
      content: [],
      metadataJson: JSON.stringify(sceneJson),
      baseUrl: 'http://localhost/'
    }),
    getExplorerInformation: async () => ({
      agent: 'dcl-scene-cli code-to-composite',
      platform: 'desktop',
      configurations: {}
    })
  },
  '~system/EngineApi': {
    crdtSendToRenderer: async ({ data }) => {
      if (data && data.length) await feed(new Uint8Array(data))
      return { data: [] }
    },
    crdtGetState: async () => ({
      hasEntities: false,
      data: existingCrdt ? [...initialCrdtState(), existingCrdt] : initialCrdtState()
    }),
    crdtGetMessageFromRenderer: async () => ({ data: [] }),
    sendBatch: async () => ({ events: [] }),
    subscribe: async () => ({}),
    unsubscribe: async () => ({}),
    isServer: async () => ({ isServer: false })
  },
  '~system/Scene': {
    getSceneInfo: async () => ({
      cid: '',
      metadata: JSON.stringify(sceneJson),
      baseUrl: 'http://localhost/',
      contents: []
    })
  },
  '~system/EnvironmentApi': {
    isPreviewMode: async () => ({ isPreview: true }),
    getPlatform: async () => ({ platform: 'desktop' }),
    areUnsafeRequestAllowed: async () => ({ status: false }),
    getCurrentRealm: async () => ({
      currentRealm: { domain: 'http://localhost', layer: '', serverName: 'code-to-composite', displayName: 'code-to-composite' }
    }),
    getExplorerConfiguration: async () => ({ clientUri: '', configurations: {} }),
    getDecentralandTime: async () => ({ seconds: 43200 })
  },
  '~system/UserIdentity': {
    getUserData: async () => ({
      data: {
        userId: '0x0000000000000000000000000000000000000000',
        displayName: 'code-to-composite',
        hasConnectedWeb3: false,
        version: 1,
        publicKey: null
      }
    }),
    getUserPublicKey: async () => ({ address: null })
  },
  '~system/Players': {
    getConnectedPlayers: async () => ({ players: [] }),
    getPlayersInScene: async () => ({ players: [] }),
    getPlayerData: async () => ({ data: undefined })
  },
  '~system/CommunicationsController': {
    send: async () => ({ data: [] }),
    sendBinary: async () => ({ data: [] })
  },
  '~system/CommsApi': { getActiveVideoStreams: async () => ({ streams: [] }) },
  '~system/SignedFetch': {
    signedFetch: async () => ({ ok: false, status: 0, statusText: 'offline', headers: {}, body: '' }),
    getHeaders: async () => ({ headers: {} })
  },
  '~system/Testing': { logTestResult: async () => ({}), plan: async () => ({}) }
}

// Anything not listed above, and any method not listed, resolves to an async
// no-op returning {} (upstream's auto-mock): scene code that touches it at
// start keeps running instead of aborting the capture.
function autoMock(base) {
  return new Proxy(base, {
    get(target, prop) {
      if (prop in target) return target[prop]
      if (prop === 'then' || typeof prop === 'symbol') return undefined
      return async () => ({})
    }
  })
}

const modules = new Map()
function hostRequire(spec) {
  if (typeof spec !== 'string' || !spec.startsWith('~system/')) {
    throw new Error('the scene requested a non-host module: ' + spec)
  }
  if (!modules.has(spec)) modules.set(spec, autoMock(STATIC[spec] ?? {}))
  return modules.get(spec)
}

// scene console output goes to stderr so the summary line owns stdout
const sceneConsole = {}
for (const k of ['log', 'info', 'debug', 'warn', 'error', 'trace']) {
  sceneConsole[k] = (...a) => console.error(...a)
}
const fakeGlobal = { require: hostRequire, console: sceneConsole }
const PREAMBLE = 'const require = globalThis.require;\n'
function loadCjs(rel) {
  const code = fs.readFileSync(path.join(root, rel), 'utf8')
  const mod = { exports: {} }
  const wrapper = new Function('globalThis', 'module', 'exports', PREAMBLE + code)
  wrapper.call(fakeGlobal, fakeGlobal, mod, mod.exports)
  return mod.exports
}

// ---------------------------------------------------------------------------
// upstream filterInspectorCompatibleComponents: drop UI entities whole, and
// the player Transform the initial state planted
// ---------------------------------------------------------------------------
function filterInspectorCompatible() {
  const uiEntities = new Set()
  for (const component of engine.componentsIter()) {
    if (!uiComponentIds.has(component.componentId)) continue
    for (const [entity] of engine.getEntitiesWith(component)) uiEntities.add(entity)
  }
  for (const component of engine.componentsIter()) {
    if (!('deleteFrom' in component)) continue
    for (const [entity] of engine.getEntitiesWith(component)) {
      if (uiEntities.has(entity) || uiComponentIds.has(component.componentId)) {
        component.deleteFrom(entity)
      }
    }
  }
  if (Transform.has(engine.PlayerEntity)) Transform.deleteFrom(engine.PlayerEntity)
  return uiEntities.size
}

// ---------------------------------------------------------------------------
// upstream migrateAssets: referenced files move under assets/scene/<Type>/
// (models into their own folder with their gltf dependencies) and every
// component path is rewritten
// ---------------------------------------------------------------------------
function assetType(file) {
  switch (path.extname(file).toLowerCase().replace(/^\./, '')) {
    case 'gltf':
    case 'glb':
      return 'Models'
    case 'png':
    case 'jpg':
    case 'jpeg':
    case 'bmp':
      return 'Images'
    case 'mp3':
    case 'wav':
    case 'ogg':
      return 'Audio'
    case 'mp4':
      return 'Video'
    default:
      return null
  }
}

function collectAssetPaths(data, into) {
  if (data === null || data === undefined) return
  if (typeof data === 'string') {
    if (data.length && assetType(data) && !/^[a-z][a-z0-9+.-]*:/i.test(data)) into.add(data)
    return
  }
  if (Array.isArray(data)) {
    for (const item of data) collectAssetPaths(item, into)
    return
  }
  if (typeof data === 'object') {
    for (const value of Object.values(data)) collectAssetPaths(value, into)
  }
}

function replaceAssetPaths(data, mapping) {
  if (data === null || data === undefined) return { data, changed: false }
  if (typeof data === 'string') {
    const to = mapping.get(data)
    return to === undefined ? { data, changed: false } : { data: to, changed: true }
  }
  if (Array.isArray(data)) {
    let changed = false
    const out = data.map((item) => {
      const r = replaceAssetPaths(item, mapping)
      changed ||= r.changed
      return r.data
    })
    return { data: out, changed }
  }
  if (typeof data === 'object') {
    let changed = false
    const out = {}
    for (const [k, v] of Object.entries(data)) {
      const r = replaceAssetPaths(v, mapping)
      changed ||= r.changed
      out[k] = r.data
    }
    return { data: out, changed }
  }
  return { data, changed: false }
}

// A glTF `uri` that names a file: a relative path, decoded, with no `..`
// segment. A URL or `data:` URI is not a file to copy; an absolute path or a
// climb out of the model's folder is not a dependency a model may have, and
// copying it would read or write outside the project.
function fileDependency(uri) {
  if (typeof uri !== 'string' || !uri || /^[a-z][a-z0-9+.-]*:/i.test(uri)) return null
  let decoded
  try {
    decoded = decodeURIComponent(uri)
  } catch {
    return null
  }
  if (path.posix.isAbsolute(decoded) || path.win32.isAbsolute(decoded)) return null
  const segments = decoded.split(/[\\/]+/)
  if (segments.some((s) => s === '' || s === '.' || s === '..')) return null
  return segments.join('/')
}

// Whether `p` is `base` or under it, once resolved.
function inside(base, p) {
  const rel = path.relative(base, path.resolve(p))
  return rel === '' || (!path.isAbsolute(rel) && rel !== '..' && !rel.startsWith('..' + path.sep))
}

// External images and buffers a .gltf (JSON) or .glb (JSON chunk) names,
// relative to the model file; data: URIs are embedded and need no copy.
// `rejected` holds the uris that are not file dependencies (see
// fileDependency), which are left as they are.
function gltfDependencies(file) {
  const none = { deps: [], rejected: [] }
  let json
  try {
    const bytes = fs.readFileSync(file)
    if (bytes.length >= 20 && bytes.readUInt32LE(0) === 0x46546c67) {
      const len = bytes.readUInt32LE(12)
      if (bytes.readUInt32LE(16) !== 0x4e4f534a) return none
      json = JSON.parse(bytes.subarray(20, 20 + len).toString('utf8'))
    } else {
      json = JSON.parse(bytes.toString('utf8'))
    }
  } catch (e) {
    console.error(`  could not read the dependencies of ${path.relative(root, file)}: ${e.message}`)
    return none
  }
  const deps = []
  const rejected = []
  for (const list of [json.images, json.buffers]) {
    for (const item of list ?? []) {
      if (typeof item.uri !== 'string' || !item.uri || item.uri.startsWith('data:')) continue
      const dep = fileDependency(item.uri)
      if (dep === null) rejected.push(item.uri)
      else deps.push(dep)
    }
  }
  return { deps, rejected }
}

function destinationFor(rel) {
  const type = assetType(rel)
  const file = path.basename(rel)
  if (type === 'Models') {
    return path.join('assets', 'scene', 'Models', path.basename(rel, path.extname(rel)), file)
  }
  return path.join('assets', 'scene', type, file)
}

function copyInto(fromAbs, toRel) {
  const toAbs = path.join(root, toRel)
  for (const p of [fromAbs, toAbs]) {
    if (!inside(root, p)) throw new Error(`${p} is outside the project`)
  }
  if (path.resolve(fromAbs) === path.resolve(toAbs)) return
  fs.mkdirSync(path.dirname(toAbs), { recursive: true })
  fs.copyFileSync(fromAbs, toAbs)
}

function migrateAssets() {
  const refs = new Set()
  for (const component of engine.componentsIter()) {
    for (const [, value] of engine.getEntitiesWith(component)) collectAssetPaths(value, refs)
  }
  const mapping = new Map()
  const missing = []
  const skipped = []
  for (const rel of refs) {
    const abs = path.isAbsolute(rel) ? rel : path.join(root, rel)
    if (!inside(root, abs) || !fs.existsSync(abs) || !fs.statSync(abs).isFile()) {
      missing.push(rel)
      continue
    }
    const posixRel = rel.split(path.sep).join('/')
    const to = destinationFor(rel).split(path.sep).join('/')
    if (to !== posixRel) {
      copyInto(abs, to)
      mapping.set(rel, to)
    }
    if (assetType(rel) !== 'Models') continue
    const { deps, rejected } = gltfDependencies(abs)
    for (const uri of rejected) skipped.push(`${posixRel}: ${uri}`)
    for (const dep of deps) {
      const depAbs = path.join(path.dirname(abs), dep)
      if (!fs.existsSync(depAbs)) continue
      const depFrom = path.posix.join(path.posix.dirname(posixRel), dep)
      const depTo = path.posix.join(path.posix.dirname(to), dep)
      if (depTo === depFrom) continue
      copyInto(depAbs, depTo)
      mapping.set(depFrom, depTo)
    }
  }
  let rewritten = 0
  for (const component of engine.componentsIter()) {
    if (!('createOrReplace' in component)) continue
    for (const [entity, value] of Array.from(engine.getEntitiesWith(component))) {
      const r = replaceAssetPaths(value, mapping)
      if (!r.changed) continue
      component.createOrReplace(entity, r.data)
      rewritten++
    }
  }
  return { moved: Array.from(mapping, ([from, to]) => ({ from, to })), missing, skipped, rewritten }
}

// ---------------------------------------------------------------------------
// upstream generateEntityNames: first matching component names the entity
// ---------------------------------------------------------------------------
function stem(p) {
  return path.parse(path.basename(p)).name
}

const NAME_RULES = [
  ['core::GltfContainer', (d) => (d?.src ? stem(d.src) : null)],
  ['core::MeshRenderer', (d) => d?.mesh?.$case ?? 'Mesh'],
  [
    'core::TextShape',
    (d) => {
      if (!d?.text) return 'Text'
      const text = String(d.text).trim()
      return `Text: ${text.length > 10 ? text.substring(0, 10) + '...' : text}`
    }
  ],
  ['core::VideoPlayer', (d) => (d?.src ? stem(d.src) : 'Video')],
  ['core::NftShape', () => 'NFT'],
  ['core::LightSource', () => 'Light'],
  ['core::AudioSource', (d) => (d?.audioClipUrl ? stem(d.audioClipUrl) : 'Audio')],
  ['core::MeshCollider', (d) => (d?.mesh?.$case ? `Mesh Collider: ${d.mesh.$case}` : 'Mesh Collider')]
]

function nameEntities() {
  const reserved = new Set([engine.RootEntity, engine.PlayerEntity, engine.CameraEntity])
  let named = 0
  for (const [componentName, rule] of NAME_RULES) {
    const component = engine.getComponentOrNull(componentName)
    if (!component) continue
    for (const [entity, value] of Array.from(engine.getEntitiesWith(component))) {
      if (reserved.has(entity) || Name.has(entity)) continue
      const name = rule(value)
      if (!name) continue
      Name.create(entity, { value: name.charAt(0).toUpperCase() + name.slice(1) })
      named++
    }
  }
  return named
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------
process.on('unhandledRejection', (e) => {
  console.error('unhandled rejection in the scene:', e)
})

if (existingCrdt) await feed(existingCrdt)

const loader = loadCjs(mainFile)
if (typeof loader.onStart !== 'function' || typeof loader.onUpdate !== 'function') {
  console.error(`${mainFile} exports no onStart/onUpdate -- not a built scene?`)
  process.exit(1)
}
try {
  await loader.onStart()
  await loader.onUpdate(0)
  await engine.update(0)
} catch (e) {
  console.error('the scene threw while starting:', e && e.stack ? e.stack : e)
  process.exit(1)
}

const uiEntities = filterInspectorCompatible()
const assets = migrateAssets()
const named = nameEntities()
await engine.update(0)

const composite = inspector.dumpEngineToComposite(engine, 'json')
const compositePath = path.join(root, 'assets', 'scene', 'main.composite')
fs.mkdirSync(path.dirname(compositePath), { recursive: true })
fs.writeFileSync(compositePath, JSON.stringify(ECS.Composite.toJson(composite), null, 2))
fs.writeFileSync(mainCrdtPath, inspector.dumpEngineToCrdtCommands(engine))

const entities = new Set()
for (const component of engine.componentsIter()) {
  for (const [entity] of engine.getEntitiesWith(component)) entities.add(entity)
}
console.log(
  JSON.stringify({
    ok: true,
    entities: entities.size,
    components: composite.components.length,
    uiEntities,
    named,
    assets: assets.moved,
    missingAssets: assets.missing,
    skippedDependencies: assets.skipped,
    rewritten: assets.rewritten
  })
)
