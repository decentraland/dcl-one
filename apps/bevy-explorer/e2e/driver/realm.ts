// A static realm of built scenes on loopback, for the visual suite: what the engine reads from a
// realm (`/about` with `scenesUrn`, the entity and its files under `/content/contents/<hash>`),
// served to the web client on another loopback origin, so with CORS and a cross-origin resource
// policy. Content is addressed by its bytes, so an unchanged file keeps its hash across runs and
// the engine's cache stays valid. A realm may also preview smart wearables (`/preview-wearables`,
// the creator preview path): their files are served by hash like a scene's, but they are no
// scene of the realm, so neither `/about` nor `/content/entities/active` lists them.
import { createHash } from 'node:crypto'
import { readdir, readFile, stat } from 'node:fs/promises'
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http'
import path from 'node:path'
import { contentType, listen, type StaticServer } from './serve.ts'

export interface ContentFile {
  file: string
  hash: string
}

export interface SceneEntity {
  id: string
  type: 'scene'
  pointers: string[]
  content: ContentFile[]
  metadata: Record<string, unknown>
}

export interface BuiltScene {
  entity: SceneEntity
  /** Local path by content hash. */
  files: Map<string, string>
}

export function contentHash(bytes: Uint8Array): string {
  return `e2e${createHash('sha256').update(bytes).digest('hex')}`
}

async function walk(dir: string, prefix: string): Promise<string[]> {
  let names: string[]
  try {
    names = await readdir(dir)
  } catch {
    return []
  }
  const out: string[] = []
  for (const name of names.sort()) {
    const full = path.join(dir, name)
    const relative = prefix ? `${prefix}/${name}` : name
    if ((await stat(full)).isDirectory()) out.push(...(await walk(full, relative)))
    else out.push(relative)
  }
  return out
}

/** The entity of a built SDK7 scene folder: `scene.json` as metadata, its `main` bundle,
 *  `main.crdt` when present, and everything under `assets/`. */
export async function buildScene(sceneDir: string): Promise<BuiltScene> {
  const metadata = JSON.parse(await readFile(path.join(sceneDir, 'scene.json'), 'utf8')) as Record<string, unknown>
  const main = typeof metadata.main === 'string' ? metadata.main : 'bin/index.js'
  const scene = metadata.scene as { base?: string; parcels?: string[] } | undefined
  const pointers = scene?.parcels ?? (scene?.base ? [scene.base] : [])
  if (pointers.length === 0) throw new Error(`${sceneDir}: scene.json names no parcels`)
  const relativeFiles = [main, 'main.crdt', ...(await walk(path.join(sceneDir, 'assets'), 'assets'))]
  const files = new Map<string, string>()
  const content: ContentFile[] = []
  for (const file of relativeFiles) {
    const full = path.join(sceneDir, file)
    let bytes: Buffer
    try {
      bytes = await readFile(full)
    } catch {
      if (file === main) throw new Error(`${sceneDir}: the bundle ${main} is missing; build the scene first`)
      continue
    }
    const hash = contentHash(bytes)
    content.push({ file, hash })
    files.set(hash, full)
  }
  const body = { type: 'scene' as const, pointers, content, metadata }
  const id = contentHash(Buffer.from(JSON.stringify(body)))
  return { entity: { id, ...body }, files }
}

/** The urn the e2e realm previews the smart-wearable fixture under. */
export const PREVIEW_WEARABLE_URN = 'urn:decentraland:off-chain:e2e:smart-wearable'

export const BODY_SHAPES = ['urn:decentraland:off-chain:base-avatars:BaseMale', 'urn:decentraland:off-chain:base-avatars:BaseFemale'] as const

/** The body-shape prefixes a smart wearable's files are listed under. */
const BODY_SHAPE_PREFIXES = ['male', 'female'] as const

export interface PreviewWearable {
  urn: string
  /** `wearable.json`. */
  metadata: Record<string, unknown>
  /** The project's files, unprefixed (`scene.json`, `bin/index.js`, `model.glb`, ...). */
  content: ContentFile[]
  /** Local path by content hash. */
  files: Map<string, string>
}

/** A built smart-wearable project for `/preview-wearables`: `scene.json`, `wearable.json`, the
 *  scene's `main` bundle and every file beside it (dcl-scene-cli's `bin/index.js` is a loader that
 *  reads the chunks next to it), `main.crdt` when present, every file the representations name,
 *  and everything under `assets/`. */
export async function buildPreviewWearable(projectDir: string, urn: string = PREVIEW_WEARABLE_URN): Promise<PreviewWearable> {
  const scene = JSON.parse(await readFile(path.join(projectDir, 'scene.json'), 'utf8')) as Record<string, unknown>
  if (scene.isPortableExperience !== true) throw new Error(`${projectDir}: scene.json is not a portable experience`)
  const metadata = JSON.parse(await readFile(path.join(projectDir, 'wearable.json'), 'utf8')) as Record<string, unknown>
  const main = typeof scene.main === 'string' ? scene.main : 'bin/index.js'
  const representations = ((metadata.data as { representations?: { mainFile?: string; contents?: string[] }[] } | undefined)?.representations ?? [])
  const modelFiles = representations.flatMap((representation) => [...(representation.contents ?? []), ...(representation.mainFile ? [representation.mainFile] : [])])
  const mainDir = path.posix.dirname(main)
  const bundle = mainDir === '.' ? [] : await walk(path.join(projectDir, mainDir), mainDir)
  const wanted = [...new Set(['scene.json', 'wearable.json', main, ...bundle, 'main.crdt', ...modelFiles, ...(await walk(path.join(projectDir, 'assets'), 'assets'))])]
  const content: ContentFile[] = []
  const files = new Map<string, string>()
  for (const file of wanted) {
    const full = path.join(projectDir, file)
    let bytes: Buffer
    try {
      bytes = await readFile(full)
    } catch {
      if (file === main) throw new Error(`${projectDir}: the bundle ${main} is missing; build the scene first`)
      if (file === 'main.crdt') continue
      throw new Error(`${projectDir}: ${file}, named by wearable.json, is missing`)
    }
    const hash = contentHash(bytes)
    content.push({ file, hash })
    files.set(hash, full)
  }
  return { urn, metadata, content, files }
}

/** `/preview-wearables`: each wearable's `wearable.json` with its id the urn and one
 *  representation for both body shapes that lists every file under `male/` and `female/`. */
export function previewWearablesDocument(origin: string, wearables: PreviewWearable[]): { ok: true; data: Record<string, unknown>[] } {
  const base = `${origin}/content/contents`
  return {
    ok: true,
    data: wearables.map((wearable) => {
      const data = (wearable.metadata.data ?? {}) as Record<string, unknown>
      const firstMain = (data.representations as { mainFile?: string }[] | undefined)?.[0]?.mainFile
      return {
        ...wearable.metadata,
        id: wearable.urn,
        data: {
          ...data,
          representations: [
            {
              bodyShapes: [...BODY_SHAPES],
              ...(firstMain ? { mainFile: `male/${firstMain}` } : {}),
              overrideHides: [],
              overrideReplaces: [],
              contents: BODY_SHAPE_PREFIXES.flatMap((prefix) =>
                wearable.content.map(({ file, hash }) => ({ key: `${prefix}/${file}`, url: `${base}/${hash}`, hash }))
              )
            }
          ]
        }
      }
    })
  }
}

export interface RealmOptions {
  scenes: BuiltScene[]
  port: number
  realmName?: string
  /** Smart wearables to list on `/preview-wearables`; without them the route answers 404, as a
   *  production catalyst does. */
  previewWearables?: PreviewWearable[]
}

export interface RealmServer extends StaticServer {
  /** The realm for `?realm=` and `--realm`: the origin, no trailing slash. */
  readonly realm: string
}

const CORS_HEADERS: Record<string, string> = {
  'access-control-allow-origin': '*',
  'access-control-allow-methods': 'GET, HEAD, POST, OPTIONS',
  'access-control-allow-headers': '*',
  'cross-origin-resource-policy': 'cross-origin',
  'cache-control': 'no-store'
}

export function aboutDocument(origin: string, scenes: BuiltScene[], realmName: string): Record<string, unknown> {
  return {
    healthy: true,
    acceptingUsers: true,
    configurations: {
      networkId: 0,
      globalScenesUrn: [],
      scenesUrn: scenes.map((scene) => `urn:decentraland:entity:${scene.entity.id}?=&baseUrl=${origin}/content/contents/`),
      realmName
    },
    content: { healthy: true, publicUrl: `${origin}/content` },
    lambdas: { healthy: true, publicUrl: `${origin}/lambdas` },
    comms: { healthy: true, protocol: 'v3', fixedAdapter: 'offline:offline' }
  }
}

function json(response: ServerResponse, status: number, body: unknown): void {
  const text = JSON.stringify(body)
  response.writeHead(status, { 'content-type': 'application/json; charset=utf-8', 'content-length': Buffer.byteLength(text) })
  response.end(text)
}

async function readBody(request: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = []
  for await (const chunk of request) chunks.push(chunk as Buffer)
  return Buffer.concat(chunks).toString('utf8')
}

export function serveRealm(options: RealmOptions): Promise<RealmServer> {
  const { scenes, port, realmName = 'e2e-visual', previewWearables } = options
  const byId = new Map(scenes.map((scene) => [scene.entity.id, scene]))
  const byHash = new Map<string, string>()
  for (const scene of scenes) for (const [hash, file] of scene.files) byHash.set(hash, file)
  for (const wearable of previewWearables ?? []) for (const [hash, file] of wearable.files) byHash.set(hash, file)
  let origin = `http://127.0.0.1:${port}`

  const handle = async (request: IncomingMessage, response: ServerResponse): Promise<void> => {
    for (const [name, value] of Object.entries(CORS_HEADERS)) response.setHeader(name, value)
    const url = new URL(request.url ?? '/', origin)
    if (request.method === 'OPTIONS') {
      response.writeHead(204).end()
      return
    }
    if (url.pathname === '/about' && (request.method === 'GET' || request.method === 'HEAD')) {
      json(response, 200, aboutDocument(origin, scenes, realmName))
      return
    }
    if (url.pathname === '/preview-wearables' && previewWearables && (request.method === 'GET' || request.method === 'HEAD')) {
      json(response, 200, previewWearablesDocument(origin, previewWearables))
      return
    }
    if (url.pathname === '/scenes') {
      json(response, 200, { scenes: [], total: 0 })
      return
    }
    if (url.pathname === '/content/entities/active' && request.method === 'POST') {
      const body = JSON.parse((await readBody(request)) || '{}') as { pointers?: string[] }
      const wanted = new Set(body.pointers ?? [])
      json(
        response,
        200,
        scenes.map((scene) => scene.entity).filter((entity) => entity.pointers.some((pointer) => wanted.has(pointer)))
      )
      return
    }
    const contents = /^\/content\/contents\/([A-Za-z0-9]+)$/.exec(url.pathname)
    if (contents && (request.method === 'GET' || request.method === 'HEAD')) {
      const hash = contents[1]!
      const scene = byId.get(hash)
      if (scene) {
        json(response, 200, scene.entity)
        return
      }
      const file = byHash.get(hash)
      if (file) {
        const bytes = await readFile(file)
        response.writeHead(200, { 'content-type': contentType(file), 'content-length': bytes.length })
        response.end(request.method === 'HEAD' ? undefined : bytes)
        return
      }
    }
    response.writeHead(404).end()
  }

  const server = createServer((request, response) => {
    handle(request, response).catch(() => {
      if (!response.headersSent) response.writeHead(500)
      response.end()
    })
  })
  return listen(server, port).then((bound) => {
    origin = `http://127.0.0.1:${bound.port}`
    return { ...bound, realm: origin }
  })
}
