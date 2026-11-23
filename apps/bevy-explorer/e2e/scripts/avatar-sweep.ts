// How frame time grows with the number of avatars on screen, for choosing the Max Avatars cap.
// Serves the web app on loopback as the suite does, opens it in scene preview as an automatic
// guest (`?preview&guest=1`, which enables `/avatar_demo`), waits until in world, then for each
// condition and each avatar count: clears the demo avatars, spawns that many, waits until every
// one is built, lets the frame settle and samples `/performance_state`. One JSON line per step
// on stdout (progress on stderr), so an agent can run it, change the engine and run it again.
//
//   pnpm run avatar-sweep [--counts 0,10,20,50,100,200,400,800] [--caps 20,all]
//     [--emotes none,dance] [--nametags on] [--layout grid|around] [--setup "<console line>"]...
//     [--seed 1|none] [--wearables <file>] [--window 10] [--settle 3] [--load-timeout 180]
//     [--realm <url>] [--position x,y] [--web-app <dir> [--port <n>] | --url <served app>]
//     [--profile <dir>] [--out <file>]
//
// The demo avatars wear random base wearables, or with `--wearables <file>` (one URN per line,
// blank lines and `#` lines ignored) random ones from those URNs: a few hundred files from the
// catalyst either way. Run after
// run that adds up to more than the catalyst allows one client, so `--profile <dir>` keeps
// Chrome's profile there: the engine's content cache then answers later runs. The cache belongs
// to the page's origin, so `--profile` takes a fixed `--port`. The last stderr line is
// `/content_request_state`: what the run downloaded and what the cache answered.
//
// Each line: {"step", "count", "built", "shown", "drawn", "loadSeconds", "cap", "emote", "nametags", "layout", "seed", "wearables", "samples",
// "gpuSamples", "fps", "frameMs", "gpuFrameMs", "engineMemoryMiB", "hiccups"} where fps, frameMs and gpuFrameMs
// are {median, p95, max} over the window's samples, and gpuSamples counts the samples that carried
// a GPU frame time. `/avatar_demo cap` is session-only, so the
// stored setting never changes; at the end the session goes back to it, and nametags and the
// demo emote to what they were.
import { appendFile, readFile, writeFile } from 'node:fs/promises'
import { basename } from 'node:path'
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
/** `/avatar_demo cap`'s highest number (`debug_avatars.rs`); `all` is no cap. */
const MAX_CAP = 1025
/** URNs per `/avatar_demo catalog` line, so one console line stays short. */
const CATALOG_BATCH = 20
/** `/avatar_demo spawn`'s population ceiling (`MAX_DEMOS`). */
const MAX_DEMOS = 1024

const { values } = parseArgs({
  options: {
    counts: { type: 'string', default: '0,10,20,50,100,200,400,800' },
    caps: { type: 'string', default: '20,all' },
    emotes: { type: 'string', default: 'none,dance' },
    nametags: { type: 'string', default: 'on' },
    layout: { type: 'string', default: 'grid' },
    seed: { type: 'string', default: '1' },
    wearables: { type: 'string' },
    setup: { type: 'string', multiple: true, default: [] },
    window: { type: 'string', default: '10' },
    settle: { type: 'string', default: '3' },
    'load-timeout': { type: 'string', default: '180' },
    realm: { type: 'string', default: DEFAULT_REALM },
    position: { type: 'string', default: '0,0' },
    'web-app': { type: 'string', default: process.env.DCL_E2E_WEB_APP || DEFAULT_WEB_APP },
    port: { type: 'string', default: '0' },
    profile: { type: 'string' },
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
const caps = list(values.caps).map((cap): number | 'all' => (cap === 'all' ? 'all' : Number(cap)))
if (caps.some((cap) => cap !== 'all' && (!Number.isInteger(cap) || cap < 1 || cap > MAX_CAP))) throw new Error(`--caps takes integers from 1 to ${MAX_CAP}, or all`)
const emotes = list(values.emotes)
const nametags = list(values.nametags).map((value) => {
  if (value !== 'on' && value !== 'off') throw new Error('--nametags takes on, off, or on,off for both')
  return value === 'on'
})
if (values.layout !== 'grid' && values.layout !== 'around') throw new Error('--layout takes grid or around')
if (values.seed !== 'none' && !/^\d{1,15}$/.test(values.seed)) throw new Error('--seed takes a whole number, or none')
const seed = values.seed === 'none' ? '' : values.seed
function port(name: string, value: string): number {
  const parsed = Number(value)
  if (!Number.isInteger(parsed) || parsed < 0 || parsed > 65535) throw new Error(`--${name} takes a port, not ${value}`)
  return parsed
}
const appPort = port('port', values.port)
// the browser keeps a page's caches by its origin, which a port picked anew each run changes
if (values.profile != null && values.url == null && appPort === 0) throw new Error('--profile needs a fixed --port, so the page keeps one origin from run to run')
const catalog = values.wearables == null
  ? []
  : (await readFile(values.wearables, 'utf8')).split('\n').map((line) => line.trim()).filter((line) => line !== '' && !line.startsWith('#'))
if (values.wearables != null && catalog.length === 0) throw new Error(`--wearables ${values.wearables} lists no URNs`)
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
  shown: number
  drawn: number
  cap: number | 'all'
  emote: string
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
  server = await serveWebApp(values['web-app'], appPort)
  base = server.url
}
const page = new URL(base)
page.searchParams.set('preview', '')
page.searchParams.set('guest', '1')
page.searchParams.set('realm', values.realm)
page.searchParams.set('position', values.position)
log(`opening ${page.href}`)
const driver = await launch({ url: page.href, headless: values.headless, args: chromeArgs(), profile: values.profile })

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

// what the session had before the sweep changed it, to put back at the end
let before: { emote: string; nametags: boolean } | undefined
let step = 0
try {
  await driver.ready({ timeout: READY_TIMEOUT_MS })
  await until('the world', IN_WORLD_TIMEOUT_MS, async () => {
    const world = await run<{ inWorld?: boolean }>('/world_state')
    return world.inWorld ? true : undefined
  })
  const first = await run<DemoReply>('/avatar_demo clear')
  if (typeof first !== 'object' || first.cap === undefined) {
    throw new Error(`/avatar_demo is unavailable (scene preview mode is needed): ${JSON.stringify(first)}`)
  }
  before = { emote: first.emote, nametags: (await run<{ visible: boolean }>('/nametags')).visible }
  if (values.out) await writeFile(values.out, '')
  await run(`/avatar_demo layout ${values.layout}`)
  await run(`/avatar_demo seed ${seed}`.trim())
  if (values.wearables != null) {
    await run('/avatar_demo catalog')
    for (let i = 0; i < catalog.length; i += CATALOG_BATCH) {
      await run(`/avatar_demo catalog ${catalog.slice(i, i + CATALOG_BATCH).join(',')}`)
    }
  }
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
          const after = await run<DemoReply>('/avatar_demo')
          const record = {
            step,
            count,
            built,
            shown: after.shown,
            drawn: after.drawn,
            loadSeconds,
            cap,
            emote,
            nametags: tags ? 'on' : 'off',
            layout: values.layout,
            seed: seed === '' ? null : Number(seed),
            wearables: values.wearables == null ? null : basename(values.wearables),
            samples: samples.length,
            gpuSamples: samples.filter((s) => typeof s.gpuFrameMs === 'number').length,
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
  await run('/avatar_demo cap').catch(() => undefined)
  if (before !== undefined) {
    await run(`/avatar_demo emote ${before.emote}`).catch(() => undefined)
    await run(`/nametags ${before.nametags}`).catch(() => undefined)
  }
  await run('/avatar_demo layout grid').catch(() => undefined)
  await run('/avatar_demo seed').catch(() => undefined)
  // what this run asked the content servers for, and what the engine's cache answered
  log(`content requests: ${JSON.stringify(await run('/content_request_state').catch(() => null))}`)
  await run('/avatar_demo catalog').catch(() => undefined)
  await driver.close()
  await server?.close()
}
