// Loading time of the live web client at fixed locations: for each location a fresh client
// (a new browser profile, so nothing is cached) is opened there and timed until the first ring
// (the player's scene and every scene within one parcel of the player) is ready; the last client
// then teleports through the locations and times each leg the same way. Writes `results.json` and
// `summary.md` to `--out` after every reading, prints the summary, and exits 0 whatever the
// readings: the workflow judges them.
//
//   pnpm run loading-time [--config loading-time.json] [--out report/loading-time] [--deadline-minutes 30]
//
// The target comes from the suite's variables, with the live client as the default:
// DCL_E2E_WEB_URL (https://decentraland.org/play), DCL_E2E_BASE_DOMAIN (decentraland.org),
// DCL_E2E_REALM (the main realm). Each client signs in as a guest; no account key is read.
import { mkdir, readFile, rename, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { parseArgs } from 'node:util'
import type { Explorer } from '../driver/explorer.ts'
import { FIRST_RING_UNSUPPORTED, type LoadingTimeResults, measureTeleport, measureTimeToWorld, parseConfig, slowLines, summaryMarkdown, teleportLegs, unmeasuredLines } from '../driver/loading-time.ts'
import { PACKAGE_ROOT, REPORT_DIR } from '../driver/paths.ts'
import { closeSession, closeSessions, openSession } from '../driver/session.ts'
import { readTarget } from '../driver/target.ts'

const LIVE_WEB_URL = 'https://decentraland.org/play'
const LIVE_BASE_DOMAIN = 'decentraland.org'
const LIVE_REALM = 'https://realm-provider-ea.decentraland.org/main'

const { values } = parseArgs({
  options: {
    config: { type: 'string', default: path.join(PACKAGE_ROOT, 'loading-time.json') },
    out: { type: 'string', default: path.join(REPORT_DIR, 'loading-time') },
    // well below the workflow step's 45 minutes: a boot abandoned at the deadline can still spend
    // its current wait (up to 5 minutes) before it returns
    'deadline-minutes': { type: 'string', default: '30' }
  }
})

const deadlineMinutes = Number(values['deadline-minutes'])
if (!(deadlineMinutes > 0)) throw new Error(`--deadline-minutes takes a positive number, not ${values['deadline-minutes']}`)
const deadline = Date.now() + deadlineMinutes * 60_000
const config = parseConfig(await readFile(values.config, 'utf8'))
const target = readTarget({
  ...process.env,
  DCL_E2E_WEB_URL: process.env.DCL_E2E_WEB_URL || LIVE_WEB_URL,
  DCL_E2E_BASE_DOMAIN: process.env.DCL_E2E_BASE_DOMAIN || LIVE_BASE_DOMAIN,
  DCL_E2E_REALM: process.env.DCL_E2E_REALM || LIVE_REALM
})
const log = (line: string): void => void process.stderr.write(`[loading-time] ${line}\n`)
const runUrl = process.env.GITHUB_RUN_ID ? `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/${process.env.GITHUB_RUN_ID}` : undefined

const results: LoadingTimeResults = {
  startedAt: new Date().toISOString(),
  webUrl: target.webUrl ?? LIVE_WEB_URL,
  baseDomain: target.baseDomain,
  realm: target.realm,
  thresholds: config.thresholds,
  worlds: [],
  teleports: []
}

/** Replace a file whole: a run killed while writing leaves the previous version. */
async function replaceFile(file: string, text: string): Promise<void> {
  await writeFile(`${file}.tmp`, text)
  await rename(`${file}.tmp`, file)
}

/** The readings so far, so a run cut short still leaves them. */
async function persist(): Promise<string> {
  const summary = summaryMarkdown(results, runUrl)
  await mkdir(values.out, { recursive: true })
  await replaceFile(path.join(values.out, 'summary.md'), summary)
  await replaceFile(path.join(values.out, 'results.json'), `${JSON.stringify({ ...results, slow: slowLines(results), unmeasured: unmeasuredLines(results) }, null, 2)}\n`)
  return summary
}

const { locations } = config
let tourClient: Explorer | undefined
// a client without firstRing fails every reading the same way: one is enough to know
let unsupported = false
try {
  for (const [index, location] of locations.entries()) {
    if (unsupported) {
      results.worlds.push({ location: location.name, position: location.position, timeToWorldMs: null, error: FIRST_RING_UNSUPPORTED })
      continue
    }
    const options = { position: location.position, signIn: 'guest' as const }
    log(`time to world at ${location.name} (${location.position})`)
    const { measurement, explorer } = await measureTimeToWorld(location, async (signal) => (await openSession({ ...options, signal }, target)).explorer, { deadline })
    results.worlds.push(measurement)
    log(JSON.stringify(measurement))
    unsupported = measurement.error === FIRST_RING_UNSUPPORTED
    // the last client stays open for the teleport tour; the others would share the GPU with it
    if (index === locations.length - 1 && explorer && !unsupported) tourClient = explorer
    else await closeSession(options, target)
    await persist()
  }
  for (const [from, to] of teleportLegs(locations)) {
    log(`teleport ${from.name} → ${to.name}`)
    const missing = unsupported ? FIRST_RING_UNSUPPORTED : `no client: the boot at ${locations.at(-1)!.name} failed`
    const leg = tourClient ? await measureTeleport(tourClient, from, to, { deadline }) : { from: from.name, to: to.name, teleportMs: null, error: missing }
    results.teleports.push(leg)
    log(JSON.stringify(leg))
    await persist()
  }
} finally {
  await closeSessions()
}

process.stdout.write(await persist())
