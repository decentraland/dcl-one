// Serves the built web app on loopback the way the benchmark runner does (local-harness/benchmark
// `server.rs`): the same headers on every response, `GET` and `HEAD` only, loopback peers only.
// The app's own service worker adds Document-Isolation-Policy to the app frame.
import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http'
import { stat } from 'node:fs/promises'
import { createReadStream } from 'node:fs'
import path from 'node:path'
import { pipeline } from 'node:stream'

export const WEB_APP_HEADERS: Record<string, string> = {
  'cross-origin-opener-policy': 'same-origin',
  'cross-origin-embedder-policy': 'require-corp',
  'cross-origin-resource-policy': 'cross-origin',
  'content-security-policy': "frame-ancestors 'self'",
  'x-frame-options': 'SAMEORIGIN',
  'cache-control': 'no-store'
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

export interface StaticServer {
  readonly url: string
  readonly port: number
  close(): Promise<void>
}

async function serveFile(root: string, request: IncomingMessage, response: ServerResponse): Promise<void> {
  for (const [name, value] of Object.entries(WEB_APP_HEADERS)) response.setHeader(name, value)
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
  const server: Server = createServer((request, response) => {
    serveFile(absoluteRoot, request, response).catch(() => {
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
