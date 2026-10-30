import assert from 'node:assert/strict'
import { createServer, type Server } from 'node:http'
import { after, before, describe, test } from 'node:test'
import { ConsoleError } from '../driver/console.ts'
import { NativeConsole, RpcFailure } from '../driver/native.ts'

interface Call {
  command: string
  polls: number
}

/** A stand-in for the automation build's loopback server: the two methods, the token, a `done`
 *  that arrives on the second poll, and `/fail` as a command that answers `Err`. */
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
        if (call.polls < 2) return reply({ result: { done: false } })
        if (call.command.startsWith('/fail')) return reply({ result: { done: true, ok: false, output: 'nope' } })
        return reply({ result: { done: true, ok: true, output: `ran ${call.command}` } })
      }
      return reply({ error: { code: -32601, message: `Method \`${method}\` not found` } })
    })
  })
  return { server, calls }
}

describe('NativeConsole', () => {
  const token = 'test-token'
  let server: Server
  let calls: Call[]
  let port: number

  before(async () => {
    ;({ server, calls } = fakeBrp(token))
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
    const address = server.address()
    port = typeof address === 'object' && address ? address.port : 0
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

  test('a wrong token is a JSON-RPC failure', async () => {
    const console_ = new NativeConsole({ port, token: 'other', pollMs: 5 })
    await assert.rejects(console_.run('/world_state'), (error: unknown) => {
      assert.ok(error instanceof RpcFailure)
      assert.equal(error.code, -32602)
      return true
    })
  })

  test('reachable says whether the server answers', async () => {
    assert.equal(await new NativeConsole({ port, token }).reachable(), true)
    assert.equal(await new NativeConsole({ port: 1, token }).reachable(), false)
  })

  test('close without a process is a no-op', async () => {
    await new NativeConsole({ port, token }).close()
  })
})
