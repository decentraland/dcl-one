import assert from 'node:assert/strict'
import { connect } from 'node:net'
import { createServer, type Server } from 'node:http'
import { after, before, describe, test } from 'node:test'
import { ConsoleError } from '../driver/console.ts'
import { NativeConsole, RpcFailure, spawnNative } from '../driver/native.ts'
import { TimeoutError } from '../driver/wait.ts'

interface Call {
  command: string
  polls: number
}

/** A stand-in for the automation build's loopback server: the two methods, the token, a `done`
 *  that arrives on the second poll, `/fail` as a command that answers `Err` and `/login_identity`
 *  as one that never answers. */
function fakeBrp(token: string): { server: Server; calls: Call[] } {
  const calls: Call[] = []
  const server = createServer((request, response) => {
    let body = ''
    request.on('data', (chunk) => (body += chunk))
    request.on('end', () => {
      const { id, method, params } = JSON.parse(body) as { id: number; method: string; params: Record<string, unknown> }
      const reply = (payload: Record<string, unknown>) => {
        response.writeHead(200, { 'content-type': 'application/json' })
        response.end(JSON.stringify({ jsonrpc: '2.0', id, ...payload }))
      }
      if (params.token !== token) return reply({ error: { code: -32602, message: 'missing or wrong "token"' } })
      if (method === 'dcl/console') {
        calls.push({ command: String(params.command), polls: 0 })
        return reply({ result: { id: calls.length } })
      }
      if (method === 'dcl/console_result') {
        const call = calls[Number(params.id) - 1]
        if (!call) return reply({ error: { code: -32602, message: 'unknown or already collected call id' } })
        call.polls++
        if (call.command.startsWith('/login_identity')) return reply({ result: { done: false } })
        if (call.polls < 2) return reply({ result: { done: false } })
        if (call.command.startsWith('/fail')) return reply({ result: { done: true, ok: false, output: 'nope' } })
        return reply({ result: { done: true, ok: true, output: `ran ${call.command}` } })
      }
      return reply({ error: { code: -32601, message: `Method \`${method}\` not found` } })
    })
  })
  return { server, calls }
}

/** A server that answers every JSON-RPC call with a method-not-found error, as a Bevy app with
 *  its own Remote Protocol on the port would. */
function otherBrp(): Server {
  return createServer((request, response) => {
    request.resume()
    request.on('end', () => {
      response.writeHead(200, { 'content-type': 'application/json' })
      response.end(JSON.stringify({ jsonrpc: '2.0', id: null, error: { code: -32601, message: 'Method `dcl/console_result` not found' } }))
    })
  })
}

function listen(server: Server): Promise<number> {
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      resolve(typeof address === 'object' && address ? address.port : 0)
    })
  })
}

/** Whether anything accepts a connection on the port. */
function accepting(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = connect({ port, host: '127.0.0.1' })
    socket.once('connect', () => {
      socket.destroy()
      resolve(true)
    })
    socket.once('error', () => resolve(false))
  })
}

describe('NativeConsole', () => {
  const token = 'test-token'
  let server: Server
  let calls: Call[]
  let port: number

  before(async () => {
    ;({ server, calls } = fakeBrp(token))
    port = await listen(server)
  })

  after(() => new Promise<void>((resolve) => server.close(() => resolve())))

  test('queues the line and polls until done', async () => {
    const console_ = new NativeConsole({ port, token, pollMs: 5 })
    assert.equal(await console_.run('world_state'), 'ran /world_state')
    assert.equal(calls[0]!.polls, 2)
  })

  test('an Err reply is a ConsoleError with the output', async () => {
    const console_ = new NativeConsole({ port, token, pollMs: 5 })
    await assert.rejects(console_.run('/fail now'), (error: unknown) => {
      assert.ok(error instanceof ConsoleError)
      assert.equal(error.output, 'nope')
      assert.equal(error.line, '/fail now')
      return true
    })
  })

  test('a command that never answers times out with its line redacted', async () => {
    const console_ = new NativeConsole({ port, token, pollMs: 5, commandTimeoutMs: 40 })
    await assert.rejects(console_.run('/login_identity eyJhIjoxfQ== --guest'), (error: unknown) => {
      assert.ok(error instanceof TimeoutError)
      assert.match(error.message, /waiting for `\/login_identity <redacted> --guest` to answer/)
      assert.doesNotMatch(error.message, /eyJhIjoxfQ/)
      return true
    })
  })

  test('a wrong token is a JSON-RPC failure', async () => {
    const console_ = new NativeConsole({ port, token: 'other', pollMs: 5 })
    await assert.rejects(console_.run('/world_state'), (error: unknown) => {
      assert.ok(error instanceof RpcFailure)
      assert.equal(error.code, -32602)
      return true
    })
  })

  test("answers tells this client's console from another server on the port", async () => {
    assert.equal(await new NativeConsole({ port, token }).answers(), true, 'the console that takes the token')
    assert.equal(await new NativeConsole({ port, token: 'other' }).answers(), false, 'a console started with another token')
    assert.equal(await new NativeConsole({ port: 1, token }).answers(), false, 'nothing on the port')
    const other = otherBrp()
    const otherPort = await listen(other)
    try {
      assert.equal(await new NativeConsole({ port: otherPort, token }).answers(), false, 'a server without the console methods')
    } finally {
      await new Promise<void>((resolve) => other.close(() => resolve()))
    }
  })

  test('a request to a server that never replies is cut off', async () => {
    const silent = createServer(() => {})
    const silentPort = await listen(silent)
    try {
      const console_ = new NativeConsole({ port: silentPort, token, requestTimeoutMs: 50 })
      await assert.rejects(console_.call('dcl/console', { command: '/world_state' }), /timeout|aborted/i)
    } finally {
      silent.closeAllConnections()
      await new Promise<void>((resolve) => silent.close(() => resolve()))
    }
  })

  test('close without a process is a no-op', async () => {
    await new NativeConsole({ port, token }).close()
  })
})

describe('spawnNative', () => {
  test('a binary that cannot be started is an error', async () => {
    await assert.rejects(spawnNative({ bin: 'no-such-client-binary', args: [], port: 1, startTimeoutMs: 1000 }), /could not be started/)
  })

  test('a client that exits before its console comes up is an error', async () => {
    // `--`: the flags spawnNative appends go to the script, not to node.
    await assert.rejects(spawnNative({ bin: process.execPath, args: ['-e', 'process.exit(3)', '--'], port: 1, startTimeoutMs: 5000 }), /exited with code 3 before its console came up/)
  })

  test('a client whose console never answers is killed when the start times out', async () => {
    // A client that holds the port but was started with another token: it refuses ours forever.
    const probe = createServer()
    const port = await listen(probe)
    await new Promise<void>((resolve) => probe.close(() => resolve()))
    const script = `
      const http = require('node:http')
      const port = Number(process.argv[process.argv.indexOf('--automation-port') + 1])
      http.createServer((request, response) => {
        request.resume()
        request.on('end', () => {
          response.writeHead(200, { 'content-type': 'application/json' })
          response.end(JSON.stringify({ jsonrpc: '2.0', id: null, error: { code: -32602, message: 'missing or wrong "token"' } }))
        })
      }).listen(port, '127.0.0.1')
      setInterval(() => {}, 1000)
    `
    await assert.rejects(spawnNative({ bin: process.execPath, args: ['-e', script, '--'], port, startTimeoutMs: 1500 }), /timed out .* the client console on loopback/)
    let open = await accepting(port)
    for (let i = 0; open && i < 20; i++) {
      await new Promise((resolve) => setTimeout(resolve, 100))
      open = await accepting(port)
    }
    assert.equal(open, false, 'the client should be gone, and the port with it')
  })
})
