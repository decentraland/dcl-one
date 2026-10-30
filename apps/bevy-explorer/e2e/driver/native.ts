// The standalone transport: every native build carries a loopback JSON-RPC server with two
// methods, `dcl/console` (queue a line, get a call id) and `dcl/console_result` (poll it until
// `done`), off unless the client is started with `--automation-token` (and `--automation-port`,
// 15702 by default). Both methods need that token.
import { spawn, type ChildProcess } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { type Console, ConsoleError, normalizeLine } from './console.ts'
import { sleep, until } from './wait.ts'

export const DEFAULT_AUTOMATION_PORT = 15702

export interface NativeOptions {
  port?: number
  token: string
  /** How often `dcl/console_result` is polled. */
  pollMs?: number
  /** How long one command may take. */
  commandTimeoutMs?: number
  host?: string
}

interface RpcError {
  code: number
  message: string
}

interface RpcResponse<T> {
  jsonrpc: '2.0'
  id: number | null
  result?: T
  error?: RpcError
}

export class RpcFailure extends Error {
  readonly code: number

  constructor(method: string, error: RpcError) {
    super(`${method}: ${error.message} (${error.code})`)
    this.name = 'RpcFailure'
    this.code = error.code
  }
}

export class NativeConsole implements Console {
  readonly kind = 'native' as const
  readonly url: string
  readonly #token: string
  readonly #pollMs: number
  readonly #commandTimeoutMs: number
  #nextId = 1
  #process: ChildProcess | undefined

  constructor(options: NativeOptions, process?: ChildProcess) {
    this.url = `http://${options.host ?? '127.0.0.1'}:${options.port ?? DEFAULT_AUTOMATION_PORT}/`
    this.#token = options.token
    this.#pollMs = options.pollMs ?? 100
    this.#commandTimeoutMs = options.commandTimeoutMs ?? 120_000
    this.#process = process
  }

  async call<T>(method: string, params: Record<string, unknown>): Promise<T> {
    const id = this.#nextId++
    const response = await fetch(this.url, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id, method, params: { ...params, token: this.#token } })
    })
    const body = (await response.json()) as RpcResponse<T>
    if (body.error) throw new RpcFailure(method, body.error)
    return body.result as T
  }

  async run(line: string): Promise<string> {
    const normalized = normalizeLine(line)
    const { id } = await this.call<{ id: number }>('dcl/console', { command: normalized })
    const result = await until(
      () => this.call<{ done: boolean; ok?: boolean; output?: string }>('dcl/console_result', { id }),
      (reply) => reply.done,
      { timeoutMs: this.#commandTimeoutMs, intervalMs: this.#pollMs, what: `\`${normalized}\` to answer` }
    )
    if (!result.ok) throw new ConsoleError(normalized, result.output ?? '')
    return result.output ?? ''
  }

  /** Whether the server answers at all (any JSON-RPC reply, an error included). */
  async reachable(): Promise<boolean> {
    try {
      await fetch(this.url, { method: 'POST', headers: { 'content-type': 'application/json' }, body: '{"jsonrpc":"2.0","id":0,"method":"dcl/console_result","params":{}}' })
      return true
    } catch {
      return false
    }
  }

  async close(): Promise<void> {
    const child = this.#process
    this.#process = undefined
    if (!child || child.exitCode !== null) return
    child.kill()
    await Promise.race([new Promise<void>((resolve) => child.once('exit', () => resolve())), sleep(10_000)])
    if (child.exitCode === null) child.kill('SIGKILL')
  }
}

export interface SpawnNativeOptions {
  /** The native client binary. */
  bin: string
  args: string[]
  port?: number
  token?: string
  env?: NodeJS.ProcessEnv
  cwd?: string
  /** How long the loopback server may take to come up. */
  startTimeoutMs?: number
}

/** Start the standalone with its loopback console on (`--automation-token`, `--automation-port`)
 *  and return a console on it. The client's stdout and stderr are inherited; pass `--log-file` in
 *  `args` for the run's log. */
export async function spawnNative(options: SpawnNativeOptions): Promise<NativeConsole> {
  const port = options.port ?? DEFAULT_AUTOMATION_PORT
  const token = options.token ?? randomBytes(16).toString('hex')
  const child = spawn(options.bin, [...options.args, '--automation-token', token, '--automation-port', String(port)], {
    cwd: options.cwd,
    env: { ...process.env, ...options.env },
    stdio: 'inherit'
  })
  const console_ = new NativeConsole({ port, token }, child)
  const exited = new Promise<never>((_, reject) => child.once('exit', (code) => reject(new Error(`the client exited with code ${code} before its console came up`))))
  await Promise.race([
    exited,
    until(() => console_.reachable(), (up) => up, { timeoutMs: options.startTimeoutMs ?? 60_000, intervalMs: 250, what: 'the client console on loopback' })
  ])
  return console_
}
