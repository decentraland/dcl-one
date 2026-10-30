// The Chrome flags the benchmark runner uses for the web client (local-harness/benchmark
// `chrome.rs`), without the ones the web driver sets itself (the profile, the window size, the
// background throttling, `--no-first-run`, `--no-default-browser-check`). Headed: a hidden tab
// pauses the engine. `--force-high-performance-gpu`: WebGPU picks the low-power GPU otherwise.
export const BENCHMARK_CHROME_ARGS: readonly string[] = [
  '--disable-search-engine-choice-screen',
  '--disable-session-crashed-bubble',
  '--hide-crash-restore-bubble',
  '--password-store=basic',
  '--use-mock-keychain',
  '--disable-hang-monitor',
  '--disable-background-mode',
  '--disable-background-networking',
  '--disable-component-update',
  '--disable-sync',
  '--disable-default-apps',
  '--disable-features=CalculateNativeWinOcclusion,LocalNetworkAccessChecks',
  '--autoplay-policy=no-user-gesture-required',
  '--force-high-performance-gpu',
  '--log-level=3'
]

/** Flags for a run: the benchmark's, then `CHROME_EXTRA_ARGS` (whitespace-separated). */
export function chromeArgs(env: NodeJS.ProcessEnv = process.env): string[] {
  const extra = (env.CHROME_EXTRA_ARGS ?? '').split(/\s+/).filter((arg) => arg !== '')
  return [...BENCHMARK_CHROME_ARGS, ...extra]
}
