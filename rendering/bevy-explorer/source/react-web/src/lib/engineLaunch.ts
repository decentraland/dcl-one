// How react-web launches the web engine (the shared EngineHost reads it through `@hud-host`): every
// launch param the engine's web param table lists, read from the entry url, the services this HUD
// resolved for itself (lib/baseDomain.ts), the ui scene (our bundled bridge scene unless a link
// overrode it), and the benchmark's switches where they count.
import type { EngineLaunch } from '@explorer-core/hostContract'
import { launchOptionsFromUrl, type LaunchOptions } from '@explorer-core/lib/webParams'
import { BASE_DOMAIN, SERVICE_OVERRIDES } from './baseDomain'
import { benchmarkSwitchesHonoured, bootMode } from './bootMode'
import { perfStreamPath } from './entryParams'
import { PAGE_DIR } from './publicUrl'
import { SYSTEM_SCENE } from './systemScene'

/**
 * What the entry url asks of the engine: its launch options and the benchmark stream (`perfStream`,
 * with `perfCsv` for the CSV). The benchmark's switches count only where `benchmark` says so,
 * which is `benchmarkSwitchesHonoured` (lib/bootMode.ts); anywhere else the engine is told nothing
 * of them: `autopilot` is left out of the launch options (absent, not `false`) and there is no stream.
 */
export function entryEngineOptions(
  params: URLSearchParams,
  benchmark = benchmarkSwitchesHonoured()
): { launch: LaunchOptions; perfStream: { path: string; csv: boolean } | null } {
  const path = benchmark ? perfStreamPath(params) : null
  return {
    launch: launchOptionsFromUrl(params, benchmark ? [] : ['autopilot']),
    perfStream: path == null ? null : { path, csv: params.has('perfCsv') }
  }
}

/** This host's engine runs without a bridge session of its own. */
export function seedBridgeSession(): void {}

export function engineLaunch(params: URLSearchParams): EngineLaunch {
  const { launch, perfStream } = entryEngineOptions(params)
  return {
    options: { ...launch, ...SERVICE_OVERRIDES, baseDomain: BASE_DOMAIN, systemScene: bootMode().systemScene ?? SYSTEM_SCENE },
    // pkg/ fetch base: the versioned CDN in prod builds (BASE_URL), the served engine dir otherwise.
    engineDir: new URL('engine', new URL(import.meta.env.BASE_URL, PAGE_DIR)).href,
    // The performance harness's browser run: a page switch, handed on by boot.js.
    pageSwitches: () => {
      if (perfStream != null) window.__bevyPerfStream = perfStream
    }
  }
}

declare global {
  interface Window {
    // Where the engine's log (and with `csv` its CSV) is streamed, a same-origin path; boot.js
    // passes it to engine_set_perf_stream and sends the bytes (engine/perf_stream.js).
    __bevyPerfStream?: { path: string; csv: boolean }
  }
}
