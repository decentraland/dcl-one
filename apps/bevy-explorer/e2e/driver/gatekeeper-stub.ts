// A comms-gatekeeper on loopback that answers as scripted: the ban-status read and the
// scene-adapter mint. Every other path is 404.
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http'
import { listen, type StaticServer } from './serve.ts'

export interface StubReply {
  status: number
  body: unknown
  headers?: Record<string, string>
}

export interface GatekeeperStub extends StaticServer {
  /** The answer to `GET /users/<address>/bans`. */
  banStatus(reply: StubReply): void
  /** The answer to `POST /get-scene-adapter`. */
  sceneAdapter(reply: StubReply): void
  /** The requests served so far, method and path. */
  readonly requests: string[]
}

/** A banned answer in the gatekeeper's shape. */
export function bannedReply(reason: string, customMessage?: string): StubReply {
  return {
    status: 200,
    body: {
      data: {
        isBanned: true,
        ban: {
          id: 'e2e-ban',
          bannedAddress: '0x' + '0'.repeat(40),
          bannedBy: '0x' + '0'.repeat(40),
          reason,
          customMessage: customMessage ?? null,
          bannedAt: '2026-01-01T00:00:00.000Z',
          expiresAt: null,
          liftedAt: null,
          liftedBy: null,
          createdAt: '2026-01-01T00:00:00.000Z'
        }
      }
    }
  }
}

export const NOT_BANNED: StubReply = { status: 200, body: { data: { isBanned: false } } }

/** A refusal in the gatekeeper's shape: a 403 with the reason, as it refuses a banned wallet. */
export function refusal(status: number, error: string, headers?: Record<string, string>): StubReply {
  return { status, body: { error }, headers }
}

function send(response: ServerResponse, reply: StubReply): void {
  const text = JSON.stringify(reply.body)
  response.writeHead(reply.status, {
    'content-type': 'application/json; charset=utf-8',
    'content-length': Buffer.byteLength(text),
    'access-control-allow-origin': '*',
    ...reply.headers
  })
  response.end(text)
}

const shared = new Map<number, Promise<GatekeeperStub>>()

/** The one stub on `port`, started on first use; every spec file shares it, since the runner keeps
 *  them in one process. */
export function sharedGatekeeperStub(port: number): Promise<GatekeeperStub> {
  let stub = shared.get(port)
  if (!stub) {
    stub = serveGatekeeperStub(port).catch((error: unknown) => {
      shared.delete(port)
      throw error
    })
    shared.set(port, stub)
  }
  return stub
}

/** Close every shared stub. */
export async function closeGatekeeperStubs(): Promise<void> {
  const open = [...shared.values()]
  shared.clear()
  for (const stub of open) {
    try {
      await (await stub).close()
    } catch {
      // a stub that never came up
    }
  }
}

/** Serve the stub on `port` (0 picks a free one), answering not banned and a 503 adapter until told otherwise. */
export async function serveGatekeeperStub(port: number): Promise<GatekeeperStub> {
  let banStatus: StubReply = NOT_BANNED
  let sceneAdapter: StubReply = refusal(503, 'stub has no adapter', { 'retry-after': '1' })
  const requests: string[] = []
  const server = createServer((request: IncomingMessage, response: ServerResponse) => {
    const path = new URL(request.url ?? '/', 'http://127.0.0.1').pathname
    requests.push(`${request.method} ${path}`)
    request.resume()
    request.on('end', () => {
      if (request.method === 'GET' && /^\/users\/0x[0-9a-fA-F]{40}\/bans$/.test(path)) return send(response, banStatus)
      if (request.method === 'POST' && path === '/get-scene-adapter') return send(response, sceneAdapter)
      send(response, { status: 404, body: { error: 'not found' } })
    })
  })
  const bound = await listen(server, port)
  return {
    ...bound,
    url: bound.url.replace(/\/$/, ''),
    requests,
    banStatus: (reply) => (banStatus = reply),
    sceneAdapter: (reply) => (sceneAdapter = reply)
  }
}
