// A static realm of built scenes on loopback, for the visual suite: what the engine reads from a
// realm (`/about` with `scenesUrn`, the entity and its files under `/content/contents/<hash>`),
// served to the web client on another loopback origin, so with CORS and a cross-origin resource
// policy. Content is addressed by its bytes, so an unchanged file keeps its hash across runs and
// the engine's cache stays valid.
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

export interface RealmOptions {
  scenes: BuiltScene[]
  port: number
  realmName?: string
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
  const { scenes, port, realmName = 'e2e-visual' } = options
  const byId = new Map(scenes.map((scene) => [scene.entity.id, scene]))
  const byHash = new Map<string, string>()
  for (const scene of scenes) for (const [hash, file] of scene.files) byHash.set(hash, file)
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
