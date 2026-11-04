// What WebGPU adapter the browser gives a page, started the way the scenarios start it: the
// browser the web driver would run (`CHROME_PATH`, else the installed Chrome), the benchmark's
// flags and `CHROME_EXTRA_ARGS`, headed unless `DCL_E2E_HEADLESS=1`. Two lines, in seconds: what
// a boot that stops at the app's "Enable GPU acceleration" page takes a timeout to say.
//   npm run gpu-probe [-- <more browser flags>...]
import { spawn, spawnSync, type ChildProcess } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import type { Readable, Writable } from 'node:stream'
import { pathToFileURL } from 'node:url'
import { chromeArgs } from '../driver/chrome.ts'
import { webDriverModule } from '../driver/web.ts'

const ANSWER_TIMEOUT_MS = 30_000
const CLOSE_TIMEOUT_MS = 5_000
const POLL_MS = 100

/** The page's side: the app's gate asks for an adapter the same way (`react-web/src/lib/gpu.ts`). */
const PROBE = `(async () => {
  if (!navigator.gpu) return 'navigator.gpu is absent'
  const adapter = await navigator.gpu.requestAdapter()
  if (!adapter) return 'no adapter'
  const info = adapter.info ?? {}
  return 'adapter vendor=' + info.vendor + ' architecture=' + info.architecture + ' device=' + info.device +
    ' description=' + info.description + ' fallback=' + adapter.isFallbackAdapter
})()`

interface Reply {
  id?: number
  result?: Record<string, any>
  error?: { message: string }
}

/** The DevTools pipe as the web driver uses it: requests out on fd 3, replies in on fd 4, each
 *  message JSON ended by a NUL. Events are ignored. */
class Pipe {
  readonly #output: Writable
  readonly #pending = new Map<number, { resolve: (result: Record<string, any>) => void; reject: (error: Error) => void }>()
  #next = 1
  #buffered = Buffer.alloc(0)

  constructor(proc: ChildProcess) {
    this.#output = proc.stdio[3] as Writable
    const input = proc.stdio[4] as Readable
    input.on('data', (chunk: Buffer) => {
      this.#buffered = Buffer.concat([this.#buffered, chunk])
      for (let end = this.#buffered.indexOf(0); end !== -1; end = this.#buffered.indexOf(0)) {
        const text = this.#buffered.subarray(0, end).toString('utf8')
        this.#buffered = this.#buffered.subarray(end + 1)
        this.#receive(text)
      }
    })
    input.on('error', () => {})
    this.#output.on('error', () => {})
  }

  send(method: string, params: Record<string, unknown> = {}, sessionId?: string): Promise<Record<string, any>> {
    const id = this.#next++
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject })
      this.#output.write(`${JSON.stringify({ id, method, params, sessionId })}\0`, (error) => {
        if (error) {
          this.#pending.delete(id)
          reject(error)
        }
      })
    })
  }

  #receive(text: string): void {
    let reply: Reply
    try {
      reply = JSON.parse(text) as Reply
    } catch {
      return
    }
    if (reply.id === undefined) return
    const pending = this.#pending.get(reply.id)
    if (!pending) return
    this.#pending.delete(reply.id)
    if (reply.error) pending.reject(new Error(reply.error.message))
    else pending.resolve(reply.result ?? {})
  }
}

function within<T>(work: Promise<T>, ms: number, what: string): Promise<T> {
  let timer: NodeJS.Timeout
  const late = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${what} within ${ms} ms`)), ms)
  })
  return Promise.race([work, late]).finally(() => clearTimeout(timer))
}

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms))

/** The browser's name and the page's answer. */
async function ask(pipe: Pipe, url: string): Promise<{ product: string; answer: string }> {
  const { product } = await pipe.send('Browser.getVersion')
  const { targetId } = await pipe.send('Target.createTarget', { url })
  const { sessionId } = await pipe.send('Target.attachToTarget', { targetId, flatten: true })
  const evaluate = (expression: string) => pipe.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }, sessionId)
  // the probe page, loaded: an evaluation in the document it replaces would have no secure context
  for (;;) {
    const loaded = await evaluate(`location.protocol === 'file:' && document.readyState === 'complete'`).catch(() => undefined)
    if (loaded?.result?.value === true) break
    await sleep(POLL_MS)
  }
  const { result, exceptionDetails } = await evaluate(PROBE)
  if (exceptionDetails) return { product, answer: `error: ${exceptionDetails.exception?.description ?? exceptionDetails.text}` }
  return { product, answer: String(result?.value) }
}

async function close(proc: ChildProcess, pipe: Pipe, exited: Promise<void>): Promise<void> {
  pipe.send('Browser.close').catch(() => {})
  const exitedWithin = (ms: number) => within(exited, ms, 'the browser did not exit').then(() => true, () => false)
  if (!(await exitedWithin(CLOSE_TIMEOUT_MS))) {
    if (proc.pid !== undefined && process.platform === 'win32') spawnSync('taskkill', ['/pid', String(proc.pid), '/T', '/F'], { stdio: 'ignore' })
    proc.kill('SIGKILL')
    await exitedWithin(CLOSE_TIMEOUT_MS)
  }
  for (const stream of proc.stdio) stream?.destroy()
}

async function main(): Promise<void> {
  const browser = (await webDriverModule()).findChrome()
  if (!browser) throw new Error('no browser: install Chrome or set CHROME_PATH')
  const headless = process.env.DCL_E2E_HEADLESS === '1'
  const flags = [...chromeArgs(), ...process.argv.slice(2)]
  const dir = await mkdtemp(path.join(tmpdir(), 'dcl-gpu-probe-'))
  const page = path.join(dir, 'probe.html')
  await writeFile(page, '<!doctype html><meta charset="utf-8"><title>WebGPU probe</title>\n')
  const proc = spawn(
    browser,
    [
      '--remote-debugging-pipe',
      `--user-data-dir=${path.join(dir, 'profile')}`,
      '--no-first-run',
      '--no-default-browser-check',
      '--no-startup-window',
      '--window-size=1600,900',
      ...(headless ? ['--headless=new'] : []),
      ...flags
    ],
    { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] }
  )
  const started = Date.now()
  const exited = new Promise<void>((resolve) => proc.once('exit', () => resolve()))
  // an exit before the answer is the answer: the browser handed off to another or refused a flag
  const gone = exited.then(() => {
    throw new Error(`the browser exited with code ${proc.exitCode} after ${Date.now() - started} ms`)
  })
  const pipe = new Pipe(proc)
  let product = '?'
  let answer: string
  try {
    ;({ product, answer } = await within(Promise.race([ask(pipe, pathToFileURL(page).href), gone]), ANSWER_TIMEOUT_MS, 'the browser did not answer'))
  } catch (error) {
    answer = `no answer: ${error instanceof Error ? error.message : String(error)}`
    process.exitCode = 1
  }
  console.log(`gpu-probe: ${browser} (${product}), ${headless ? 'headless' : 'headed'}, flags: ${flags.join(' ') || '(none)'}`)
  console.log(`webgpu: ${answer}`)
  await close(proc, pipe, exited)
  await rm(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 }).catch(() => {})
}

main().catch((error: unknown) => {
  console.error(`gpu-probe: ${error instanceof Error ? error.message : String(error)}`)
  process.exitCode = 1
})
