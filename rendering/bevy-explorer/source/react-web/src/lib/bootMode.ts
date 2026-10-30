import { isLoopbackHost } from './loopbackHost'

// The interrelated "how does this page boot" decisions, derived from the entry URL in ONE
// place. They cut across App.tsx (render gate), EngineHost (engine boot config) and
// useEngineSession (login/destination flow) — deriving them independently from raw params in
// each spot lets the combinations drift apart.
//
//   ?hud=0         render only the engine — no React HUD chrome (the sites /discover embed,
//                  which frames the scene itself; pair with ?guest=1 for a no-UI guest preview)
//   ?guest=1       skip the sign-in screen with an auto guest-login
//   ?systemScene=  debug: substitute the super-user ui scene (a url/dir, or "none" for the
//                  engine's builtin ui). The scene owns login/UI in-engine (pre-react
//                  behavior), so this implies a hidden HUD and no React login at all.
//   ?autopilot     the benchmark run (decentraland/performance-testing), unattended: no sign-in
//                  screen, lobby, destination picker or welcome page. Signs in with the local
//                  guest and nothing else, never a stored account; the engine ends the run. A
//                  flag: `?autopilot` and `?autopilot=` both count as set. Only where
//                  `benchmarkSwitchesHonoured` says so (below): anywhere else it does nothing.
// The engine-bound params (?portables=, ?imposterSource=, ?pulseServer=, …) are not boot-mode
// decisions: they come from the engine's web param table (lib/webParams.ts) and are handed over
// as-is by EngineHost.
export interface BootMode {
  /** explicit ?systemScene= override; null = the default bridge scene drives */
  systemScene: string | null
  /** render only the engine — no HUD chrome, no sign-in/loading overlays */
  hideHud: boolean
  /** skip the sign-in screen: auto guest-login, no React login at all (the ui scene owns it), or
   *  the autopilot's local guest */
  autoLogin: 'guest' | 'scene-owned' | 'autopilot' | null
}

/** The HUD is hosted by the native (CEF) client: a native build entered with ?native=1. */
export function isNativeHud(): boolean {
  return typeof __NATIVE_HUD__ !== 'undefined' && __NATIVE_HUD__ && new URLSearchParams(location.search).get('native') === '1'
}

/** The benchmark's switches (docs/guidelines/benchmark.md): the unattended run, and where its log
 *  and CSV are streamed. */
export const BENCHMARK_PARAMS = ['autopilot', 'perfStream', 'perfCsv']

/**
 * Whether this page acts on the benchmark's switches: on a loopback host, where the harness serves
 * the web app, and in the native client, where `--autopilot` is the user's own flag. Anywhere else
 * they do nothing at all, because from a link they would sign in and play unattended in a
 * visitor's browser (`autopilot`) or send the session's log to the site serving the page
 * (`perfStream`). `bootMode` takes no `autopilot` boot mode, `EngineHost` hands the engine no
 * `autopilot` and starts no stream, and lib/launchGate.ts has nothing to ask about.
 */
export function benchmarkSwitchesHonoured({ host = location.hostname, native = isNativeHud() } = {}): boolean {
  return native || isLoopbackHost(host)
}

// Computed per call, not a module const, so tests can set location.search before mounting.
export function bootMode(): BootMode {
  const q = new URLSearchParams(location.search)
  const systemScene = q.get('systemScene') || null
  return {
    systemScene,
    hideHud: q.get('hud') === '0' || systemScene != null,
    autoLogin:
      q.get('guest') === '1'
        ? 'guest'
        : systemScene != null
          ? 'scene-owned'
          : q.has('autopilot') && benchmarkSwitchesHonoured()
            ? 'autopilot'
            : null
  }
}
