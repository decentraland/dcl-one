// How frame time grows with the number of avatars on screen, for choosing the Max Avatars cap.
// Serves the web app on loopback as the suite does, opens it in scene preview as an automatic
// guest (`?preview&guest=1`, which enables `/avatar_demo`), waits until in world, then for each
// condition and each avatar count: clears the demo avatars, spawns that many, waits until every
// one is built, lets the frame settle and samples `/performance_state`. One JSON line per step
// on stdout (progress on stderr), so an agent can run it, change the engine and run it again.
//
//   pnpm run avatar-sweep [--counts 0,10,20,50,100,200,400,800] [--caps 20,all]
//     [--emotes none,dance] [--nametags on] [--layout grid|around] [--setup "<console line>"]...
//     [--window 10] [--settle 3] [--load-timeout 180]
//     [--realm <url>] [--position x,y] [--web-app <dir> | --url <served app>] [--out <file>]
//
// Each line: {"step", "count", "built", "loadSeconds", "cap", "emote", "nametags", "layout", "samples",
// "fps", "frameMs", "gpuFrameMs", "engineMemoryMiB", "hiccups"} where fps, frameMs and gpuFrameMs
// are {median, p95, max} over the window's samples. The session's Max Avatars is restored at the
// end. `/avatar_demo cap` is session-only, so the stored setting never changes.
import { appendFile, writeFile } from 'node:fs/promises'
import { parseArgs } from 'node:util'
import { chromeArgs } from '../driver/chrome.ts'
import { DEFAULT_WEB_APP } from '../driver/paths.ts'
import { serveWebApp, type StaticServer } from '../driver/serve.ts'
import { webDriverModule } from '../driver/web.ts'

const DEFAULT_REALM = 'https://worlds-content-server.decentraland.zone/world/genesispng.dcl.eth'
const SAMPLE_MS = 250
const POLL_MS = 500
const READY_TIMEOUT_MS = 180_000
const IN_WORLD_TIMEOUT_MS = 300_000
/** `/avatar_demo cap`'s ceiling: every demo avatar and the player (`debug_avatars.rs`). */
const CAP_ALL = 1025
/** `/avatar_demo spawn`'s population ceiling (`MAX_DEMOS`). */
const MAX_DEMOS = 1024

const { values } = parseArgs({
  options: {
    counts: { type: 'string', default: '0,10,20,50,100,200,400,800' },
    caps: { type: 'string', default: '20,all' },
    emotes: { type: 'string', default: 'none,dance' },
    nametags: { type: 'string', default: 'on' },
    layout: { type: 'string', default: 'grid' },
    setup: { type: 'string', multiple: true, default: [] },
    window: { type: 'string', default: '10' },
    settle: { type: 'string', default: '3' },
    'load-timeout': { type: 'string', default: '180' },
    realm: { type: 'string', default: DEFAULT_REALM },
    position: { type: 'string', default: '0,0' },
    'web-app': { type: 'string', default: process.env.DCL_E2E_WEB_APP || DEFAULT_WEB_APP },
    url: { type: 'string' },
    out: { type: 'string' },
    headless: { type: 'boolean', default: false }
  }
})

function list(value: string): string[] {
  return value.split(',').map((item) => item.trim()).filter((item) => item !== '')
}

function seconds(name: string, value: string): number {
  const parsed = Number(value)
  if (!Number.isFinite(parsed) || parsed < 0) throw new Error(`--${name} takes seconds, not ${value}`)
  return parsed
}

const counts = list(values.counts).map(Number)
if (counts.some((n) => !Number.isInteger(n) || n < 0 || n > MAX_DEMOS)) throw new Error(`--counts takes integers from 0 to ${MAX_DEMOS}`)
const caps = list(values.caps).map((cap) => (cap === 'all' ? CAP_ALL : Number(cap)))
if (caps.some((cap) => !Number.isInteger(cap) || cap < 1 || cap > CAP_ALL)) throw new Error(`--caps takes integers from 1 to ${CAP_ALL}, or all`)
const emotes = list(values.emotes)
const nametags = list(values.nametags).map((value) => {
  if (value !== 'on' && value !== 'off') throw new Error('--nametags takes on, off or both')
  return value === 'on'
})
if (values.layout !== 'grid' && values.layout !== 'around') throw new Error('--layout takes grid or around')
const windowMs = seconds('window', values.window) * 1000
const settleMs = seconds('settle', values.settle) * 1000
const loadTimeoutMs = seconds('load-timeout', values['load-timeout']) * 1000

const log = (line: string): void => void process.stderr.write(`${line}\n`)
const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms))

interface Stats {
  median: number | null
  p95: number | null
  max: number | null
}

function stats(samples: (number | null | undefined)[]): Stats {
  const sorted = samples.filter((n): n is number => typeof n === 'number').sort((a, b) => a - b)
  if (sorted.length === 0) return { median: null, p95: null, max: null }
  const at = (q: number): number => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))] ?? 0
  return { median: at(0.5), p95: at(0.95), max: sorted[sorted.length - 1] ?? null }
}

interface DemoReply {
  count: number
  built: number
  cap: number
}

interface PerformanceReply {
  frameMs: number
  fps: number | null
  gpuFrameMs: number | null
  engineMemoryMiB: number | null
  hiccups: number | null
}

let server: StaticServer | undefined
const { launch } = await webDriverModule()
let base = values.url
if (base == null) {
  server = await serveWebApp(values['web-app'], 0)
  base = server.url
}
const page = new URL(base)
page.searchParams.set('preview', '')
page.searchParams.set('guest', '1')
page.searchParams.set('realm', values.realm)
page.searchParams.set('position', values.position)
log(`opening ${page.href}`)
const driver = await launch({ url: page.href, headless: values.headless, args: chromeArgs() })

async function run<T>(line: string): Promise<T> {
  const reply = await driver.console(line, { timeout: 60_000 })
  try {
    return JSON.parse(reply) as T
  } catch {
    return reply as T
  }
}

async function until<T>(what: string, timeoutMs: number, probe: () => Promise<T | undefined>): Promise<T> {
  const deadline = Date.now() + timeoutMs
  for (;;) {
    const value = await probe().catch(() => undefined)
    if (value !== undefined) return value
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`)
    await sleep(POLL_MS)
  }
}

let originalCap: number | undefined
let step = 0
try {
  await driver.ready({ timeout: READY_TIMEOUT_MS })
  await until('the world', IN_WORLD_TIMEOUT_MS, async () => {
    const world = await run<{ inWorld?: boolean }>('/world_state')
    return world.inWorld ? true : undefined
  })
  const first = await run<DemoReply>('/avatar_demo clear')
  if (typeof first !== 'object' || typeof first.cap !== 'number') {
    throw new Error(`/avatar_demo is unavailable (scene preview mode is needed): ${JSON.stringify(first)}`)
  }
  originalCap = first.cap
  if (values.out) await writeFile(values.out, '')
  await run(`/avatar_demo layout ${values.layout}`)
  // console lines that set the build up for this run, such as an engine tunable under comparison
  for (const line of values.setup) log(`setup ${line}: ${JSON.stringify(await run(line))}`)

  for (const cap of caps) {
    for (const emote of emotes) {
      for (const tags of nametags) {
        await run(`/avatar_demo cap ${cap}`)
        await run(`/avatar_demo emote ${emote}`)
        await run(`/nametags ${tags}`)
        for (const count of counts) {
          step += 1
          await run('/avatar_demo clear')
          const started = Date.now()
          if (count > 0) await run(`/avatar_demo spawn ${count}`)
          const built = await until(`${count} built avatars`, loadTimeoutMs, async () => {
            const reply = await run<DemoReply>('/avatar_demo')
            return reply.count === count && reply.built >= count ? reply.built : undefined
          }).catch((error: unknown) => {
            log(String(error))
            return -1
          })
          const loadSeconds = built < 0 ? null : (Date.now() - started) / 1000
          await sleep(settleMs)
          const samples: PerformanceReply[] = []
          const windowEnd = Date.now() + windowMs
          while (Date.now() < windowEnd) {
            samples.push(await run<PerformanceReply>('/performance_state'))
            await sleep(SAMPLE_MS)
          }
          const last = samples[samples.length - 1]
          const record = {
            step,
            count,
            built,
            loadSeconds,
            cap: cap === CAP_ALL ? 'all' : cap,
            emote,
            nametags: tags ? 'on' : 'off',
            layout: values.layout,
            samples: samples.length,
            fps: stats(samples.map((s) => s.fps)),
            frameMs: stats(samples.map((s) => s.frameMs)),
            gpuFrameMs: stats(samples.map((s) => s.gpuFrameMs)),
            engineMemoryMiB: last?.engineMemoryMiB ?? null,
            hiccups: last?.hiccups ?? null
          }
          const line = JSON.stringify(record)
          process.stdout.write(`${line}\n`)
          if (values.out) await appendFile(values.out, `${line}\n`)
          log(`step ${step}: ${count} avatars, cap ${record.cap}, ${emote}, nametags ${record.nametags}: ` +
            `frame ${record.frameMs.median} ms (p95 ${record.frameMs.p95}), load ${loadSeconds ?? 'timed out'} s`)
        }
      }
    }
  }
} finally {
  await run('/avatar_demo clear').catch(() => undefined)
  if (originalCap !== undefined) await run(`/avatar_demo cap ${originalCap}`).catch(() => undefined)
  await run('/nametags true').catch(() => undefined)
  await run('/avatar_demo layout grid').catch(() => undefined)
  await driver.close()
  await server?.close()
}
