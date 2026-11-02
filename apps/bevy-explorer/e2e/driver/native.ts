// The standalone transport: every native build carries a loopback JSON-RPC server with two
// methods, `dcl/console` (queue a line, get a call id) and `dcl/console_result` (poll it until
// `done`), off unless the client is started with `--automation-token` (and `--automation-port`,
// 15702 by default). Both methods need that token.
import { spawn, type ChildProcess } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { type Console, ConsoleError, normalizeLine, redactLine } from './console.ts'
import { sleep, until } from './wait.ts'

export const DEFAULT_AUTOMATION_PORT = 15702

export interface NativeOptions {
  port?: number
  token: string
  /** How often `dcl/console_result` is polled. */
  pollMs?: number
  /** How long one command may take. */
  commandTimeoutMs?: number
  /** How long one request to the server may take: a client that hangs never settles a call. */
  requestTimeoutMs?: number
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

/** The server's answer to `dcl/console_result` for a call id it does not hold: the one reply that
 *  only this client's console, holding the run's token, gives. */
const UNKNOWN_CALL_ID = /unknown or already collected call id/

function exited(child: ChildProcess): boolean {
  return child.exitCode !== null || child.signalCode !== null
}

export class NativeConsole implements Console {
  readonly kind = 'native' as const
  readonly url: string
  readonly #token: string
  readonly #pollMs: number
  readonly #commandTimeoutMs: number
  readonly #requestTimeoutMs: number
  #nextId = 1
  #process: ChildProcess | undefined

  constructor(options: NativeOptions, process?: ChildProcess) {
    this.url = `http://${options.host ?? '127.0.0.1'}:${options.port ?? DEFAULT_AUTOMATION_PORT}/`
    this.#token = options.token
    this.#pollMs = options.pollMs ?? 100
    this.#commandTimeoutMs = options.commandTimeoutMs ?? 120_000
    this.#requestTimeoutMs = options.requestTimeoutMs ?? 10_000
    this.#process = process
  }

  async call<T>(method: string, params: Record<string, unknown>): Promise<T> {
    const id = this.#nextId++
    const response = await fetch(this.url, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id, method, params: { ...params, token: this.#token } }),
      signal: AbortSignal.timeout(this.#requestTimeoutMs)
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
      { timeoutMs: this.#commandTimeoutMs, intervalMs: this.#pollMs, what: `\`${redactLine(normalized)}\` to answer` }
    )
    if (!result.ok) throw new ConsoleError(normalized, result.output ?? '')
    return result.output ?? ''
  }

  /** Whether this client's console answers at `url`: a server that takes the run's token and
   *  knows the console methods. A client of an earlier run, or another session's on the same
   *  port, was started with another token and refuses it; any other server has no such method. */
  async answers(): Promise<boolean> {
    try {
      await this.call('dcl/console_result', { id: 0 })
      return true
    } catch (error) {
      return error instanceof RpcFailure && UNKNOWN_CALL_ID.test(error.message)
    }
  }

  async close(): Promise<void> {
    const child = this.#process
    this.#process = undefined
    if (!child || exited(child)) return
    child.kill()
    await Promise.race([new Promise<void>((resolve) => child.once('exit', () => resolve())), sleep(10_000)])
    if (!exited(child)) child.kill('SIGKILL')
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
 *  `args` for the run's log. A client that cannot start, exits, or never answers within
 *  `startTimeoutMs` is an error, and the child is gone by then. */
export async function spawnNative(options: SpawnNativeOptions): Promise<NativeConsole> {
  const port = options.port ?? DEFAULT_AUTOMATION_PORT
  const token = options.token ?? randomBytes(16).toString('hex')
  const child = spawn(options.bin, [...options.args, '--automation-token', token, '--automation-port', String(port)], {
    cwd: options.cwd,
    env: { ...process.env, ...options.env },
    stdio: 'inherit'
  })
  const console_ = new NativeConsole({ port, token }, child)
  const gone = new Promise<never>((_, reject) => {
    child.once('error', (error) => reject(new Error(`the client ${options.bin} could not be started: ${error.message}`, { cause: error })))
    child.once('exit', (code, signal) => reject(new Error(`the client exited with ${signal ? `signal ${signal}` : `code ${code}`} before its console came up`)))
  })
  try {
    await Promise.race([
      gone,
      until(() => console_.answers(), (up) => up, { timeoutMs: options.startTimeoutMs ?? 60_000, intervalMs: 250, what: 'the client console on loopback' })
    ])
  } catch (error) {
    await console_.close()
    throw error
  }
  return console_
}
