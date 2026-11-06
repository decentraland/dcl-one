// Serves the built web app on loopback with the isolation headers the app's own dev server sets
// per path (react-web `vite.config.ts`, which the app's service worker mirrors for a deployed
// host): the shell (`/`, `index.html`) is not isolated, or it refuses to start rather than take
// the app into its process; the app frame (`app.html`, and the scene server's
// `engine/headless.html`) isolates itself with Document-Isolation-Policy; everything else
// carries the opener and embedder policies. The bundled bridge scene's realm document is served
// with this origin's absolute content url, as Sites serves it. `GET` and `HEAD` only, loopback
// peers only.
import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http'
import { readFile, stat } from 'node:fs/promises'
import { createReadStream } from 'node:fs'
import path from 'node:path'
import { pipeline } from 'node:stream'

/** On every response: the origin's own pages may frame each other and no other site may, and
 *  nothing is cached across the builds served on this origin (the benchmark runner's `server.rs`). */
export const COMMON_HEADERS: Record<string, string> = {
  'cross-origin-resource-policy': 'cross-origin',
  'content-security-policy': "frame-ancestors 'self'",
  'x-frame-options': 'SAMEORIGIN',
  'cache-control': 'no-store'
}

/** On the pages the app isolates for itself: the app frame, and the scene server's frame. */
export const APP_ISOLATION_HEADERS: Record<string, string> = {
  'document-isolation-policy': 'isolate-and-credentialless'
}

/** On everything but the shell and the app's own pages. `credentialless`, as the dev server and
 *  the worker set it: the engine pulls scene assets from catalysts that send no CORP. */
export const ISOLATION_HEADERS: Record<string, string> = {
  'cross-origin-opener-policy': 'same-origin',
  'cross-origin-embedder-policy': 'credentialless'
}

const SHELL_PAGES = new Set(['/', '/index.html'])
const APP_PAGES = new Set(['/app.html', '/engine/headless.html'])

/** The headers a request path gets, by the dev server's rule. */
export function headersFor(requestPath: string): Record<string, string> {
  const requested = requestPath.split('?')[0] ?? ''
  if (SHELL_PAGES.has(requested)) return COMMON_HEADERS
  if (APP_PAGES.has(requested)) return { ...COMMON_HEADERS, ...APP_ISOLATION_HEADERS }
  return { ...COMMON_HEADERS, ...ISOLATION_HEADERS }
}

const CONTENT_TYPES: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.glb': 'model/gltf-binary',
  '.gltf': 'model/gltf+json',
  '.bin': 'application/octet-stream',
  '.txt': 'text/plain; charset=utf-8',
  '.wat': 'text/plain; charset=utf-8',
  '.crdt': 'application/octet-stream'
}

export function contentType(file: string): string {
  return CONTENT_TYPES[path.extname(file).toLowerCase()] ?? 'application/octet-stream'
}

/** The file a request path maps to under `root`, or undefined when the path is refused. */
export function resolveFile(root: string, requestPath: string): string | undefined {
  let decoded: string
  try {
    decoded = decodeURIComponent(requestPath.split('?')[0] ?? '')
  } catch {
    return undefined
  }
  if (decoded.includes('..') || decoded.includes('\\') || decoded.includes(':') || /[\x00-\x1f]/.test(decoded)) return undefined
  const relative = decoded.endsWith('/') ? `${decoded}index.html` : decoded
  const file = path.join(root, relative)
  const rootWithSep = root.endsWith(path.sep) ? root : root + path.sep
  return file.startsWith(rootWithSep) ? file : undefined
}

/** The bundled bridge scene's realm document. The web build exports it with a root-relative
 *  content url (`apps/bevy-explorer/build-web.py`: `baseUrl=/bridge-scene/static/`), which the
 *  engine takes verbatim and cannot parse ("relative URL without a base": the bridge scene never
 *  loads, and the HUD reports it unreachable). Sites serves the document with the deployment's
 *  absolute address (`apps/sites/src/routes/routes/play_.bridge.about.ts`); so does this server. */
export const BRIDGE_ABOUT_PATH = '/bridge-scene/static/BevyExplorerUI/about'
const BRIDGE_CONTENT_PATH = '/bridge-scene/static/'
const RELATIVE_BRIDGE_URN = /^urn:decentraland:entity:(baf[a-z2-7]+)\?=&baseUrl=\/bridge-scene\/static\/$/

/** The realm document with its scene's root-relative content url made absolute under `origin`.
 *  A document whose url is absolute already is left as it is (a build exported with
 *  `SCENE_BASE_URL`); one with a root-relative url the build is not known to write is refused,
 *  since served as it is it would fail in the engine as "relative URL without a base" with
 *  nothing in the suite's own logs. A document without one scene urn is not the bridge's. */
export function bridgeAbout(about: string, origin: string): string {
  const realm = JSON.parse(about) as { configurations?: { scenesUrn?: unknown } }
  const urns = realm.configurations?.scenesUrn
  if (!Array.isArray(urns) || urns.length !== 1 || typeof urns[0] !== 'string') return about
  const match = RELATIVE_BRIDGE_URN.exec(urns[0])
  if (!match) {
    if (/[?&]baseUrl=\//.test(urns[0])) throw new Error(`the bridge scene's realm names a root-relative content url this server does not know how to serve: ${urns[0]}`)
    return about
  }
  const content = new URL(BRIDGE_CONTENT_PATH, origin).href
  return JSON.stringify({ ...realm, configurations: { ...realm.configurations, scenesUrn: [`urn:decentraland:entity:${match[1]}?=&baseUrl=${content}`] } })
}

export interface StaticServer {
  readonly url: string
  readonly port: number
  close(): Promise<void>
}

async function serveFile(root: string, origin: () => string, request: IncomingMessage, response: ServerResponse): Promise<void> {
  for (const [name, value] of Object.entries(headersFor(request.url ?? '/'))) response.setHeader(name, value)
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    response.writeHead(405, { allow: 'GET, HEAD' }).end()
    return
  }
  const file = resolveFile(root, request.url ?? '/')
  if (!file) {
    response.writeHead(400).end()
    return
  }
  let info
  try {
    info = await stat(file)
  } catch {
    response.writeHead(404).end()
    return
  }
  if (!info.isFile()) {
    response.writeHead(404).end()
    return
  }
  // the file the request resolved to, as `resolveFile` decodes and normalises the path
  if (file === path.join(root, ...BRIDGE_ABOUT_PATH.split('/').filter(Boolean))) {
    let body: string
    try {
      body = bridgeAbout(await readFile(file, 'utf8'), origin())
    } catch (error) {
      // said here, where the boot log is, not left to the engine's "relative URL without a base"
      console.error(`[e2e] ${BRIDGE_ABOUT_PATH}: ${error instanceof Error ? error.message : String(error)}`)
      response.writeHead(503).end()
      return
    }
    response.writeHead(200, { 'content-type': 'application/json; charset=utf-8', 'content-length': Buffer.byteLength(body), 'accept-ranges': 'none' })
    response.end(request.method === 'HEAD' ? undefined : body)
    return
  }
  response.writeHead(200, { 'content-type': contentType(file), 'content-length': info.size, 'accept-ranges': 'none' })
  if (request.method === 'HEAD') {
    response.end()
    return
  }
  // A read that fails (the file gone since the stat, unreadable) or a client that goes away ends
  // both streams, where `pipe` would leave the descriptor open and the error uncaught.
  pipeline(createReadStream(file), response, () => {})
}

/** Serve `root` on `http://127.0.0.1:<port>/` (port 0 picks a free one). */
export function serveWebApp(root: string, port: number): Promise<StaticServer> {
  const absoluteRoot = path.resolve(root)
  // the bound port: a request only comes once the server listens
  const origin = (): string => {
    const address = server.address()
    if (typeof address !== 'object' || address == null) throw new Error('the server is not listening')
    return `http://127.0.0.1:${address.port}`
  }
  const server: Server = createServer((request, response) => {
    serveFile(absoluteRoot, origin, request, response).catch(() => {
      if (!response.headersSent) response.writeHead(500)
      response.end()
    })
  })
  return listen(server, port)
}

export function listen(server: Server, port: number): Promise<StaticServer> {
  return new Promise((resolve, reject) => {
    server.once('error', reject)
    server.listen(port, '127.0.0.1', () => {
      const address = server.address()
      const bound = typeof address === 'object' && address ? address.port : port
      resolve({
        url: `http://127.0.0.1:${bound}/`,
        port: bound,
        close: () =>
          new Promise<void>((done) => {
            server.closeAllConnections()
            server.close(() => done())
          })
      })
    })
  })
}
