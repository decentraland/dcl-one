#!/usr/bin/env node
// Executes the pinned config provider, both handler registrations, transport
// capacity/overflow, and native uWS sends over a temporary Unix socket.
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { req, load, logs, revision } = require('./social-service-voice-source-cases.cjs')
const { createRecordConfigComponent } = req('@well-known-components/env-config-provider')
const { registerWsHandler } = load('controllers/handlers/uws/ws-handler.ts')
const { registerStreamWsHandler } = load('controllers/handlers/uws/stream-ws-handler.ts')
const { createUWebSocketTransport } = load('utils/UWebSocketTransport.ts')
const mitt = req('mitt').default || req('mitt')
const FakeTimers = req('@sinonjs/fake-timers')
const uws = req('uWebSockets.js')
const WebSocket = req('ws')
const encoded = value => Number.isFinite(value) ? value : String(value)

async function nativeProbe(number) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'social-ws-backpressure-'))
  const socketPath = path.join(directory, 'socket')
  let listener, client
  try {
    return await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('Native backpressure probe timed out')), 5000)
      const app = uws.App().ws('/', {
        idleTimeout: 0,
        maxBackpressure: number,
        open(socket) {
          // At most 8 MiB, irrespective of the configured threshold. The next
          // byte distinguishes a small wrapped threshold from zero/large ones.
          let buffered = 0
          for (let attempt = 0; attempt < 8 && !buffered; attempt++) {
            socket.send(Buffer.alloc(1024 * 1024), true)
            buffered = socket.getBufferedAmount()
          }
          const probeResult = socket.send(Buffer.from([1]), true)
          const result = {
            saw_native_backpressure: buffered > 131072,
            next_send_result: probeResult
          }
          socket.close()
          clearTimeout(timer)
          if (!result.saw_native_backpressure) reject(Error('Cannot establish native backpressure'))
          else resolve(result)
        }
      })
      app.listen_unix(socket => {
        if (!socket) { clearTimeout(timer); reject(Error('Cannot listen on Unix socket')); return }
        listener = socket
        client = new WebSocket(`ws+unix://${socketPath}:/`)
        client.on('error', () => {})
      }, socketPath)
    })
  } finally {
    client?.terminate()
    if (listener) uws.us_listen_socket_close(listener)
    fs.rmSync(directory, { recursive: true, force: true })
  }
}

async function capture(raw) {
  const config = createRecordConfigComponent(raw === null ? {} : { WS_MAX_BACKPRESSURE: raw })
  let number
  try { number = (await config.getNumber('WS_MAX_BACKPRESSURE')) ?? 131072 }
  catch (error) { return { raw, error: error.message } }
  const handlers = {}
  const components = {
    config, logs, metrics: { increment() {}, observe() {} },
    uwsServer: { app: { ws(route, behavior) { handlers[route] = behavior.maxBackpressure } } },
    fetcher: {}, rpcServer: {}, tracing: {}, wsPool: {}, wsAdmission: {}, streamSessions: {}
  }
  await registerWsHandler(components)
  await registerStreamWsHandler(components)
  const clock = FakeTimers.install({ now: 0, toFake: ['setTimeout', 'clearTimeout', 'Date'] })
  let transport, capacity, sendCalls = 0
  const errors = []
  let overflowAt = null
  try {
    transport = await createUWebSocketTransport({
      getUserData: () => ({ isConnected: true }), getBufferedAmount: () => 0,
      send() { sendCalls++; return 2 }, close() {}, end() {}
    }, mitt(), { ...components, logs: { getLogger: () => ({
      ...logs.getLogger(),
      debug(message, fields) { if (message === 'Queue size configuration') capacity = fields.finalMaxQueueSize }
    }) } })
    transport.on('error', error => { errors.push(error.message); transport.close() })
    // Finite default queue capacities are <= 1000. No proportional allocation
    // or unbounded queue is needed to capture the first rejected admission.
    for (let index = 1; index <= 1001 && !errors.length; index++) {
      transport.sendMessage(Uint8Array.from([1]))
      if (errors.length) overflowAt = index
    }
  } finally { transport?.close(); clock.uninstall() }
  const native = await nativeProbe(handlers['/'])
  const nativeLimit = number >>> 0
  const supported = Number.isFinite(number) && nativeLimit !== 0
  if (native.next_send_result !== (nativeLimit > 0 && nativeLimit <= 131072 ? 2 : 0)) {
    throw Error(`Unexpected native threshold behavior for ${raw}: ${JSON.stringify(native)}`)
  }
  return {
    raw, number: encoded(number),
    handler_numbers: Object.fromEntries(Object.entries(handlers).map(([route, value]) => [route, encoded(value)])),
    native_u32: nativeLimit, native, capacity: encoded(capacity),
    send_calls: sendCalls, overflow_at: overflowAt, errors,
    bounded_target_supported: supported,
    target_gap: supported ? null : 'Native zero disables the buffer bound; finite nonzero native thresholds are required.'
  }
}

async function main() {
  const cases = []
  for (const raw of [null, '1024bytes', '131072.25tail', '4294967297.5suffix',
    '4295098368.25', '-4294966272.5', '-1.5', '+1.9tail', '0', '0.5', '-0.5',
    '4294967296', '-4294967296', 'Infinity', '-Infinity', '1e999', '1e300', '0x10', 'NaN', '']) {
    cases.push(await capture(raw))
  }
  fs.writeFileSync(process.argv[3], JSON.stringify({
    baseline: { repository: 'decentraland/social-service-ea', revision },
    runtime: { node: process.version, uws_source_commit: fs.readFileSync(path.join(path.dirname(req.resolve('uWebSockets.js')), 'source_commit'), 'utf8').trim() },
    scope: 'Executed env provider, both actual WS handler registrations, ordinary transport capacity and overflow, and native uWS sends through Unix sockets with at most 8 MiB. native_u32 is the Int32Value-to-unsigned-field conversion; native probes independently distinguish small wrapped thresholds from disabled/large thresholds, not exact kernel byte counts. Nonfinite/zero native limits remain unsupported by the bounded target.',
    cases
  }, null, 2) + '\n')
  process.stdout.write(`Captured ${cases.length} source backpressure cases\n`)
}
main().catch(error => { console.error(error); process.exitCode = 1 })
