import * as net from 'net'
import {
  createServerComponent,
  Router
} from '../../../../packages/@dcl/sdk-commands/node_modules/@well-known-components/http-server'
import { upgradeWebSocketResponse } from '../../../../packages/@dcl/sdk-commands/node_modules/@well-known-components/http-server/dist/ws'
import { createRecordConfigComponent } from '../../../../packages/@dcl/sdk-commands/node_modules/@well-known-components/env-config-provider'
import WebSocket, { WebSocketServer } from '../../../../packages/@dcl/sdk-commands/node_modules/ws'
import {
  FORBIDDEN_RESPONSE,
  isLocalOrigin,
  isLocalRequest,
  isLoopbackAddress,
  localOnly,
  PEER_ADDRESS_HEADER,
  stampPeerAddress
} from '../../../../packages/@dcl/sdk-commands/src/commands/start/server/access'

function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = net.createServer()
    server.once('error', reject)
    server.listen(0, '127.0.0.1', () => {
      const address = server.address() as net.AddressInfo
      server.close(() => resolve(address.port))
    })
  })
}

function request(headers: Record<string, string>) {
  return { headers: new Headers(headers) } as any
}

describe('isLoopbackAddress', () => {
  it.each(['127.0.0.1', '127.8.9.10', '::1', '::ffff:127.0.0.1'])('should accept %s', (address) => {
    expect(isLoopbackAddress(address)).toBe(true)
  })

  it.each(['192.168.1.20', '::ffff:192.168.1.20', '10.0.0.1', '', undefined, '127.0.0.1.evil'])(
    'should reject %s',
    (address) => {
      expect(isLoopbackAddress(address)).toBe(false)
    }
  )
})

describe('isLocalOrigin', () => {
  it.each([null, '', 'null', 'file:///app/index.html', 'http://localhost:8000', 'https://127.0.0.1', 'http://[::1]:3000'])(
    'should accept %s',
    (origin) => {
      expect(isLocalOrigin(origin)).toBe(true)
    }
  )

  it.each([
    'http://scene.example',
    'http://localhost.example:8000',
    'http://192.168.1.5:8000',
    'http://127.0.0.1.example',
    'ws://localhost:8000'
  ])('should reject %s', (origin) => {
    expect(isLocalOrigin(origin)).toBe(false)
  })
})

describe('isLocalRequest', () => {
  it('should accept a loopback peer without an Origin', () => {
    expect(isLocalRequest(request({ [PEER_ADDRESS_HEADER]: '127.0.0.1' }))).toBe(true)
  })

  it('should accept a loopback peer from a local page', () => {
    expect(isLocalRequest(request({ [PEER_ADDRESS_HEADER]: '::1', origin: 'http://localhost:8000' }))).toBe(true)
  })

  it('should fail closed when the peer address was not stamped', () => {
    expect(isLocalRequest(request({}))).toBe(false)
  })

  it('should reject a LAN peer and a loopback peer driven by a remote page', () => {
    expect(isLocalRequest(request({ [PEER_ADDRESS_HEADER]: '192.168.1.20' }))).toBe(false)
    expect(isLocalRequest(request({ [PEER_ADDRESS_HEADER]: '127.0.0.1', origin: 'http://scene.example' }))).toBe(
      false
    )
  })
})

describe('localOnly', () => {
  it('should pass a local request on and refuse the others', async () => {
    const next = jest.fn(async () => ({ status: 200 }))
    expect(await localOnly({ request: request({ [PEER_ADDRESS_HEADER]: '127.0.0.1' }) } as any, next)).toEqual({
      status: 200
    })
    expect(next).toHaveBeenCalledTimes(1)
    expect(await localOnly({ request: request({ [PEER_ADDRESS_HEADER]: '10.0.0.7' }) } as any, next)).toBe(
      FORBIDDEN_RESPONSE
    )
    expect(next).toHaveBeenCalledTimes(1)
  })
})

describe('stampPeerAddress', () => {
  let url: string
  let stop: () => Promise<void>
  let sockets: WebSocketServer
  let stamp: boolean

  async function start() {
    const port = await freePort()
    const logger = { log: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn(), debug: jest.fn() }
    const config = createRecordConfigComponent({ HTTP_SERVER_PORT: String(port), HTTP_SERVER_HOST: '127.0.0.1' })
    sockets = new WebSocketServer({ noServer: true })
    const server = await createServerComponent<object>(
      { config, ws: sockets, logs: { getLogger: () => logger } as any },
      { cors: {} }
    )
    const router = new Router<object>()
    router.get('/peer', async (ctx) => ({ body: ctx.request.headers.get(PEER_ADDRESS_HEADER) ?? 'missing' }))
    router.get('/gated', localOnly, async () => ({ body: 'ok' }))
    router.get('/socket', async (ctx) => {
      if (!isLocalRequest(ctx.request)) return FORBIDDEN_RESPONSE
      return upgradeWebSocketResponse((ws: any) => ws.send('hello'))
    })
    server.use(router.allowedMethods())
    server.use(router.middleware())
    if (stamp) await stampPeerAddress(server)
    await server.start!({} as any)
    url = `http://127.0.0.1:${port}`
    stop = () => server.stop!()
  }

  afterEach(async () => {
    sockets.close()
    await stop()
  })

  describe('when the preview server stamps its peers', () => {
    beforeEach(async () => {
      stamp = true
      await start()
    })

    it('should write the peer address over whatever the client sent', async () => {
      const spoofed = await fetch(`${url}/peer`, { headers: { [PEER_ADDRESS_HEADER]: '10.0.0.1' } })
      expect(await spoofed.text()).toBe('127.0.0.1')
      const plain = await fetch(`${url}/peer`)
      expect(await plain.text()).toBe('127.0.0.1')
    })

    it('should answer a loopback request and refuse one driven by a remote page', async () => {
      expect((await fetch(`${url}/gated`)).status).toBe(200)
      expect((await fetch(`${url}/gated`, { headers: { origin: 'http://localhost:3000' } })).status).toBe(200)
      const refused = await fetch(`${url}/gated`, { headers: { origin: 'http://scene.example' } })
      expect(refused.status).toBe(403)
      expect(await refused.json()).toEqual(FORBIDDEN_RESPONSE.body)
    })

    it('should gate the websocket upgrade the same way', async () => {
      const wsUrl = `${url.replace('http', 'ws')}/socket`
      const greeting = await new Promise<string>((resolve, reject) => {
        const socket = new WebSocket(wsUrl)
        socket.on('message', (data) => {
          socket.close()
          resolve(data.toString())
        })
        socket.on('error', reject)
      })
      expect(greeting).toBe('hello')

      const status = await new Promise<number>((resolve, reject) => {
        const socket = new WebSocket(wsUrl, { origin: 'http://scene.example' })
        socket.on('unexpected-response', (_req, res) => resolve(res.statusCode ?? 0))
        socket.on('open', () => reject(new Error('the remote page was let in')))
        socket.on('error', reject)
      })
      expect(status).toBe(403)
    })
  })

  describe('when the peers are not stamped', () => {
    beforeEach(async () => {
      stamp = false
      await start()
    })

    it('should refuse every request', async () => {
      expect(await (await fetch(`${url}/peer`)).text()).toBe('missing')
      expect((await fetch(`${url}/gated`)).status).toBe(403)
      expect((await fetch(`${url}/gated`, { headers: { [PEER_ADDRESS_HEADER]: '127.0.0.1' } })).status).toBe(200)
    })
  })
})
