// What GPU the browser gives the scenarios, started the way they start it: the browser the web
// driver would run (`CHROME_PATH`, else the installed Chrome), the benchmark's flags and
// `CHROME_EXTRA_ARGS`, headed unless `DCL_E2E_HEADLESS=1`. In seconds, what a boot that stops at
// the app's "Enable GPU acceleration" page takes a timeout to say:
//   - `webgpu:` the adapter a page gets from `navigator.gpu.requestAdapter()`, as the app's gate
//     asks for it;
//   - `gpu:` every adapter the browser knows, with its LUID and which one its GPU process runs on
//     (chrome://gpu, saved whole under `report/`), the WebGPU and WebGL feature status, the
//     problems the browser lists, and Dawn's view of the adapters.
// `--adapter <vendor>` (nvidia, amd, intel, or a hex vendor id) names the adapter the scenarios
// should run on: when the browser lists it and runs on another, the probe runs once more with
// `--use-adapter-luid=<high>,<low>`, Chromium's switch for the adapter its GPU process starts on
// (Windows), and prints `adapter-luid: <high>,<low>` for the caller to pass the same way. A VM
// whose display adapter is a paravirtual one keeps its real GPU as a second adapter, and the
// browser starts on the first.
//   npm run gpu-probe [-- --adapter nvidia] [-- <more browser flags>...]
import { spawn, spawnSync, type ChildProcess } from 'node:child_process'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import type { Readable, Writable } from 'node:stream'
import { pathToFileURL } from 'node:url'
import { chromeArgs } from '../driver/chrome.ts'
import { REPORT_DIR } from '../driver/paths.ts'
import { webDriverModule } from '../driver/web.ts'

const ANSWER_TIMEOUT_MS = 30_000
const GPU_PAGE_TIMEOUT_MS = 20_000
const CLOSE_TIMEOUT_MS = 5_000
const POLL_MS = 100

const VENDORS: Record<string, string> = { nvidia: '0x10de', amd: '0x1002', intel: '0x8086' }

/** chrome://gpu's text, shadow roots included (its view is a custom element), cells and rows
 *  separated so an adapter row reads as one line. */
const GPU_PAGE_TEXT = `(() => {
  const parts = []
  const walk = (node) => {
    if (node.nodeType === Node.TEXT_NODE) { parts.push(node.textContent); return }
    if (node.shadowRoot) walk(node.shadowRoot)
    for (const child of node.childNodes) walk(child)
    if (node.nodeType !== Node.ELEMENT_NODE) return
    if (/^(TD|TH)$/.test(node.tagName)) parts.push(' ')
    else if (/^(DIV|P|LI|TR|H[1-6]|SECTION|TABLE|BR|UL|SPAN)$/.test(node.tagName)) parts.push('\\n')
  }
  walk(document.documentElement)
  return parts.join('')
})()`

/** The page's side: the app's gate asks for an adapter the same way (`library/ui/apps/hud/lib/gpu.ts`). */
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

/** An adapter line of chrome://gpu: `GPU0 VENDOR= 0x10de, DEVICE=0x1eb8, ..., LUID={0,12345} *ACTIVE*`. */
export interface GpuLine {
  line: string
  vendor: string
  luid: string
  active: boolean
}

/** What of chrome://gpu's text is worth the log. */
export interface GpuSummary {
  gpus: GpuLine[]
  features: string[]
  problems: string[]
  dawn: string[]
}

export function summarizeGpuPage(text: string): GpuSummary {
  const lines = text.split(/\r?\n/).map((line) => line.replace(/\s+/g, ' ').trim())
  const gpus: GpuLine[] = []
  let label = 'GPU?'
  for (const line of lines) {
    if (/^GPU\d+$/.test(line)) label = line
    const match = /VENDOR\s*=\s*(0x[0-9a-f]+).*LUID=\{(-?\d+),(\d+)\}(.*)$/i.exec(line)
    if (match) gpus.push({ line: `${label} ${line}`, vendor: match[1]!.toLowerCase(), luid: `${match[2]},${match[3]}`, active: /\*ACTIVE\*/.test(match[4]!) })
  }
  const features: string[] = []
  for (let i = 0; i < lines.length; i++) {
    const match = /^(WebGPU|WebGL|WebGL2|Vulkan|OpenGL|Compositing|Rasterization|Direct Rendering Display Compositor):\s*(.*)$/.exec(lines[i]!)
    if (!match) continue
    if (features.some((feature) => feature.startsWith(`${match[1]}:`))) continue // the hardware-GPU repeat
    let status = match[2]!
    for (let j = i + 1; status === '' && j < lines.length && j <= i + 3; j++) status = lines[j]!
    features.push(`${match[1]}: ${status}`)
  }
  // a section: its title, blank lines, a rule of `=`, then its lines up to the next title
  const nextText = (i: number) => {
    for (let j = i + 1; j < lines.length; j++) if (lines[j] !== '') return j
    return -1
  }
  const isRule = (i: number) => i !== -1 && /^=+$/.test(lines[i]!)
  const section = (title: string, limit: number, keep: (line: string) => boolean): string[] => {
    const start = lines.indexOf(title)
    const rule = start === -1 ? -1 : nextText(start)
    if (!isRule(rule)) return []
    const out: string[] = []
    for (let i = rule + 1; i < lines.length && out.length < limit; i++) {
      const line = lines[i]!
      if (line === '') continue
      if (isRule(nextText(i))) break
      if (keep(line)) out.push(line)
    }
    return out
  }
  // a problem's sentence, not its bullet, its bug number or its workaround ids
  const sentence = (line: string) => /\s/.test(line) && !/^(Applied Workarounds:|\d+\(http)/.test(line) && !/^[*=:,]+$/.test(line)
  return { gpus, features, problems: section('Problems Detected', 10, sentence), dawn: section('Dawn Info', 16, (line) => line !== '') }
}

/** The browser's name, the page's answer, and chrome://gpu's text. */
async function ask(pipe: Pipe, url: string): Promise<{ product: string; answer: string; gpuPage: string; refused?: string }> {
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
  const answer = exceptionDetails ? `error: ${exceptionDetails.exception?.description ?? exceptionDetails.text}` : String(result?.value)
  // chrome://gpu fills itself in once the GPU process has answered; the adapter lines come last
  const navigation = await pipe.send('Page.navigate', { url: 'chrome://gpu' }, sessionId)
  if (navigation.errorText) return { product, answer, gpuPage: '', refused: String(navigation.errorText) }
  let gpuPage = ''
  const until = Date.now() + GPU_PAGE_TIMEOUT_MS
  while (Date.now() < until) {
    const page = await evaluate(GPU_PAGE_TEXT).catch(() => undefined)
    const text = typeof page?.result?.value === 'string' ? page.result.value : ''
    if (/Graphics Feature Status/.test(text) && /LUID=|Dawn Info/.test(text)) {
      gpuPage = text
      break
    }
    gpuPage = text
    await sleep(POLL_MS * 5)
  }
  return { product, answer, gpuPage }
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

/** One browser start with `flags`: what it printed, and the chrome://gpu summary. */
async function probe(browser: string, headless: boolean, flags: string[]): Promise<GpuSummary | undefined> {
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
  let gpuPage = ''
  let refused: string | undefined
  let summary: GpuSummary | undefined
  try {
    ;({ product, answer, gpuPage, refused } = await within(Promise.race([ask(pipe, pathToFileURL(page).href), gone]), ANSWER_TIMEOUT_MS, 'the browser did not answer'))
  } catch (error) {
    answer = `no answer: ${error instanceof Error ? error.message : String(error)}`
    process.exitCode = 1
  }
  console.log(`gpu-probe: ${browser} (${product}), ${headless ? 'headless' : 'headed'}, flags: ${flags.join(' ') || '(none)'}`)
  console.log(`webgpu: ${answer}`)
  if (gpuPage) {
    summary = summarizeGpuPage(gpuPage)
    for (const gpu of summary.gpus) console.log(`gpu: ${gpu.line}`)
    for (const line of summary.features) console.log(`gpu feature: ${line}`)
    for (const line of summary.problems) console.log(`gpu problem: ${line}`)
    for (const line of summary.dawn) console.log(`dawn: ${line}`)
    const saved = path.join(REPORT_DIR, `gpu-probe-${Date.now()}.txt`)
    await mkdir(REPORT_DIR, { recursive: true })
    await writeFile(saved, `${flags.join(' ')}\n\n${gpuPage}`)
    console.log(`gpu-probe: chrome://gpu saved to ${saved}`)
  } else {
    console.log(`gpu-probe: chrome://gpu ${refused ? `refused: ${refused}` : 'did not fill in'}`)
  }
  await close(proc, pipe, exited)
  await rm(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 }).catch(() => {})
  return summary
}

function parseArguments(argv: string[]): { adapter?: string; flags: string[] } {
  const flags: string[] = []
  let adapter: string | undefined
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]!
    if (arg === '--adapter') {
      const value = argv[++i]
      if (!value) throw new Error('--adapter needs a vendor: nvidia, amd, intel or a hex id')
      adapter = (VENDORS[value.toLowerCase()] ?? value).toLowerCase()
      if (!/^0x[0-9a-f]+$/.test(adapter)) throw new Error(`--adapter ${value}: not a vendor name or a hex id`)
    } else {
      flags.push(arg)
    }
  }
  return { adapter, flags }
}

async function main(): Promise<void> {
  const browser = (await webDriverModule()).findChrome()
  if (!browser) throw new Error('no browser: install Chrome or set CHROME_PATH')
  const headless = process.env.DCL_E2E_HEADLESS === '1'
  const { adapter, flags: extra } = parseArguments(process.argv.slice(2))
  const flags = [...chromeArgs(), ...extra]
  const summary = await probe(browser, headless, flags)
  if (!adapter || !summary) return
  const wanted = summary.gpus.find((gpu) => gpu.vendor === adapter)
  if (!wanted) {
    console.log(`gpu-probe: the browser lists no adapter of vendor ${adapter}`)
    return
  }
  if (wanted.active) {
    console.log(`gpu-probe: the browser already runs on the ${adapter} adapter (LUID ${wanted.luid})`)
    console.log(`adapter-luid: ${wanted.luid}`)
    return
  }
  console.log(`gpu-probe: the browser runs on another adapter; once more on the ${adapter} one, LUID ${wanted.luid}`)
  await probe(browser, headless, [...flags, `--use-adapter-luid=${wanted.luid}`])
  console.log(`adapter-luid: ${wanted.luid}`)
}

main().catch((error: unknown) => {
  console.error(`gpu-probe: ${error instanceof Error ? error.message : String(error)}`)
  process.exitCode = 1
})
