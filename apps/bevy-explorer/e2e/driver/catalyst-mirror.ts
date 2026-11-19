// A catalyst on loopback that keeps what it has served: a measurement that loads the same
// wearables in every session asks the real catalyst for each file once, ever, and at a gentle
// rate. Content files (`GET /content/contents/<hash>`) and active-entity lookups
// (`POST /content/entities/active`) are kept in a directory and answered from it afterwards;
// every other request is passed through and not kept (its body as text). The explorer is pointed at it with its
// `catalyst` service override. Only that explorer's page is served: a request names loopback as
// its host, and one from a browser page comes from the page's origin.
import { createHash } from 'node:crypto'
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http'
import { mkdir, readFile, rename, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { listenLocalhost, type StaticServer } from './serve.ts'

export const DEFAULT_UPSTREAM = 'https://peer.decentraland.org'
/** Requests to the upstream at once, and the pause after each: at most about ten a second. */
const UPSTREAM_CONCURRENCY = 4
const UPSTREAM_PAUSE_MS = 350
/** A refused or failed upstream request is tried this many times, waiting longer each time. */
const UPSTREAM_TRIES = 5
const RETRY_BASE_MS = 2_000
const RETRY_MAX_MS = 60_000

const CONTENT_FILE = /^\/content\/contents\/([A-Za-z0-9]{10,128})$/
const ACTIVE_ENTITIES = '/content/entities/active'

/** The longest request body read: an active-entities lookup for a crowd is a few kilobytes. */
const MAX_BODY_BYTES = 1024 * 1024
/** The names a request may give as its host, without the port. */
const LOOPBACK_HOSTS = new Set(['localhost', '127.0.0.1', '[::1]'])

/** What lets the page at `pageOrigin`, and no other, read the mirror's answers. */
function corsHeaders(pageOrigin: string | undefined): Record<string, string> {
  if (pageOrigin === undefined) return {}
  return {
    'access-control-allow-origin': pageOrigin,
    vary: 'origin',
    'access-control-allow-methods': 'GET, HEAD, POST, OPTIONS',
    'access-control-allow-headers': '*',
    'access-control-max-age': '86400',
    'cross-origin-resource-policy': 'cross-origin'
  }
}

class BodyTooLarge extends Error {}

export interface MirrorStats {
  /** Answered from the directory. */
  kept: number
  /** Asked of the upstream and kept. */
  fetched: number
  /** Passed through, not kept. */
  passed: number
  /** Upstream requests that were refused or failed, retries included. */
  refused: number
}

export interface CatalystMirror extends StaticServer {
  readonly stats: MirrorStats
}

interface Upstream {
  status: number
  contentType: string
  body: Buffer
}

const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms))

/** The wait before try `attempt` (from 1) of a refused request: the upstream's `retry-after`
 *  when it gives one, else doubling from the base, both capped. */
export function retryDelayMs(attempt: number, retryAfter: string | null): number {
  const asked = retryAfter == null ? Number.NaN : Number(retryAfter) * 1000
  const wait = Number.isFinite(asked) && asked > 0 ? asked : RETRY_BASE_MS * 2 ** (attempt - 1)
  return Math.min(wait, RETRY_MAX_MS)
}

/** An active-entities request as it is sent upstream, and the file its answer is kept in: the
 *  same pointers or ids in any order ask the same thing, and so do pointers in any case. Ids
 *  keep theirs, which is part of a content id. Undefined for a body that is not such a request. */
export function activeEntitiesLookup(body: string): { key: string; body: string } | undefined {
  let parsed: unknown
  try {
    parsed = JSON.parse(body)
  } catch {
    return undefined
  }
  if (typeof parsed !== 'object' || parsed === null) return undefined
  const { pointers, ids } = parsed as { pointers?: unknown; ids?: unknown }
  const strings = (value: unknown, each: (item: string) => string): string[] | undefined =>
    Array.isArray(value) && value.every((item) => typeof item === 'string') ? [...new Set((value as string[]).map(each))].sort() : undefined
  const canonical = { pointers: strings(pointers, (pointer) => pointer.toLowerCase()), ids: strings(ids, (id) => id) }
  if (canonical.pointers === undefined && canonical.ids === undefined) return undefined
  const asked = JSON.stringify(canonical)
  return { key: createHash('sha256').update(asked).digest('hex'), body: asked }
}

function readBody(request: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = []
    let size = 0
    request.on('data', (chunk: Buffer) => {
      size += chunk.length
      if (size > MAX_BODY_BYTES) reject(new BodyTooLarge())
      else chunks.push(chunk)
    })
    request.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')))
    request.on('error', reject)
  })
}

async function keep(file: string, body: Buffer): Promise<void> {
  await mkdir(path.dirname(file), { recursive: true })
  // written beside the file and moved into place, so a stopped run leaves no partial file
  const partial = `${file}.${process.pid}.${Date.now()}.partial`
  await writeFile(partial, body)
  await rename(partial, file)
}

/** Serve the mirror on loopback at `port` (0 picks a free one), keeping files under `directory`.
 *  `pageOrigin` is the one web origin whose pages may use it; without it, no page may. */
export async function serveCatalystMirror(directory: string, port = 0, upstream = DEFAULT_UPSTREAM, pageOrigin?: string): Promise<CatalystMirror> {
  const root = path.resolve(directory)
  const cors = corsHeaders(pageOrigin)
  const origin = upstream.replace(/\/$/, '')
  const stats: MirrorStats = { kept: 0, fetched: 0, passed: 0, refused: 0 }

  // one queue for everything asked of the upstream
  let running = 0
  const waiting: (() => void)[] = []
  async function slot<T>(work: () => Promise<T>): Promise<T> {
    if (running >= UPSTREAM_CONCURRENCY) await new Promise<void>((resolve) => waiting.push(resolve))
    running += 1
    try {
      return await work()
    } finally {
      // the slot stays taken for the pause; the answer does not wait for it
      void sleep(UPSTREAM_PAUSE_MS).then(() => {
        running -= 1
        waiting.shift()?.()
      })
    }
  }

  function ask(target: string, init: RequestInit): Promise<Upstream> {
    return slot(async () => {
      for (let attempt = 1; ; attempt++) {
        let reply: Response | undefined
        try {
          reply = await fetch(`${origin}${target}`, init)
        } catch {
          // a network failure is retried like a refusal
        }
        if (reply && reply.status !== 429 && reply.status < 500) {
          return { status: reply.status, contentType: reply.headers.get('content-type') ?? 'application/octet-stream', body: Buffer.from(await reply.arrayBuffer()) }
        }
        // a refusal's body is not read, so it is released for the connection to be used again
        await reply?.body?.cancel().catch(() => undefined)
        stats.refused += 1
        if (attempt >= UPSTREAM_TRIES) {
          return { status: reply?.status ?? 502, contentType: 'text/plain; charset=utf-8', body: Buffer.from('the upstream catalyst refused the request') }
        }
        await sleep(retryDelayMs(attempt, reply?.headers.get('retry-after') ?? null))
      }
    })
  }

  // a file asked for twice before the first answer arrives is fetched once
  const pending = new Map<string, Promise<Upstream>>()
  async function kept(file: string, contentType: string, fetchIt: () => Promise<Upstream>): Promise<Upstream> {
    try {
      const body = await readFile(file)
      stats.kept += 1
      return { status: 200, contentType, body }
    } catch {
      // not kept yet
    }
    let fetching = pending.get(file)
    if (!fetching) {
      fetching = fetchIt()
        .then(async (reply) => {
          if (reply.status === 200) {
            await keep(file, reply.body)
            stats.fetched += 1
          }
          return reply
        })
        .finally(() => pending.delete(file))
      pending.set(file, fetching)
    }
    const reply = await fetching
    return reply.status === 200 ? { ...reply, contentType } : reply
  }

  async function answer(request: IncomingMessage): Promise<Upstream> {
    const url = new URL(request.url ?? '/', 'http://localhost')
    const method = request.method ?? 'GET'
    const file = CONTENT_FILE.exec(url.pathname)
    if (file && (method === 'GET' || method === 'HEAD')) {
      const hash = file[1] ?? ''
      return kept(path.join(root, 'contents', hash), 'application/octet-stream', () => ask(url.pathname, {}))
    }
    const body = method === 'GET' || method === 'HEAD' ? undefined : await readBody(request)
    if (method === 'POST' && url.pathname === ACTIVE_ENTITIES && body !== undefined) {
      const lookup = activeEntitiesLookup(body)
      if (lookup) {
        const init = { method, body: lookup.body, headers: { 'content-type': 'application/json' } }
        return kept(path.join(root, 'active', `${lookup.key}.json`), 'application/json; charset=utf-8', () => ask(url.pathname, init))
      }
    }
    stats.passed += 1
    const contentType = request.headers['content-type']
    return ask(`${url.pathname}${url.search}`, { method, body, headers: contentType ? { 'content-type': contentType } : {} })
  }

  const handler = (request: IncomingMessage, response: ServerResponse): void => {
    // a page of another origin, or a name that only resolves to loopback, is not the explorer
    const host = (request.headers.host ?? '').replace(/:\d+$/, '').toLowerCase()
    const from = request.headers.origin
    if (!LOOPBACK_HOSTS.has(host) || (from !== undefined && from !== pageOrigin)) {
      request.resume()
      response.writeHead(403).end()
      return
    }
    if (request.method === 'OPTIONS') {
      response.writeHead(204, cors).end()
      return
    }
    answer(request)
      .then((reply) => {
        response.writeHead(reply.status, { ...cors, 'content-type': reply.contentType, 'content-length': reply.body.length })
        response.end(request.method === 'HEAD' ? undefined : reply.body)
      })
      .catch((error: unknown) => {
        if (error instanceof BodyTooLarge) {
          // the rest of the body is not read: the connection ends with the answer
          response.writeHead(413, { ...cors, connection: 'close' }).end(() => request.destroy())
          return
        }
        if (!response.headersSent) response.writeHead(502, cors)
        response.end()
      })
  }
  const listening = await listenLocalhost(() => createServer(handler), port)
  return { ...listening, url: `http://localhost:${listening.port}`, stats }
}
