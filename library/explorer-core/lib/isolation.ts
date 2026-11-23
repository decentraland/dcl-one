// The engine needs SharedArrayBuffer, so the app document (app.html) must be cross-origin isolated.
// Where the browser supports Document-Isolation-Policy (Chrome 137+), app.html isolates itself in a
// process of its own, and the shell page that embeds it stays unisolated: the sign-in key the shell
// holds stays out of the process that runs scene code. A browser without that policy (Firefox,
// Safari) can only isolate whole pages. There the app reports that it is not isolated, the shell
// chooses page isolation, and the service worker, registered with `?isolation=page`, gives the shell
// and the app COOP and COEP credentialless. The shell and the app then share one process.

const KEY = 'dcl-shell-isolation'
const PAGE = 'page'

/** Whether this browser uses page isolation (chosen earlier, kept in localStorage). */
export function pageIsolation(): boolean {
  try {
    return localStorage.getItem(KEY) === PAGE
  } catch {
    return false
  }
}

/** The service worker's URL for the chosen isolation, in the directory `dir`. */
export function serviceWorkerUrl(dir: string | URL): URL {
  const url = new URL('service_worker.js', dir)
  if (pageIsolation()) url.searchParams.set('isolation', PAGE)
  return url
}

/** Whether a worker of the chosen isolation served this page, with the isolation that worker gives. */
export function servedByWorker(): boolean {
  const worker = navigator.serviceWorker?.controller
  if (worker == null) return false
  const page = pageIsolation()
  return (new URL(worker.scriptURL).searchParams.get('isolation') === PAGE) === page && crossOriginIsolated === page
}

/** Whether the shell may mount the app: an isolated shell only with page isolation. */
export function shellMayMount(): boolean {
  return !crossOriginIsolated || pageIsolation()
}

/** Chooses page isolation after the app reported that it is not isolated. False when that cannot
 *  help: page isolation is on already, or the browser has no service worker or no storage. */
export function choosePageIsolation(): boolean {
  if (pageIsolation() || crossOriginIsolated || !('serviceWorker' in navigator)) return false
  try {
    localStorage.setItem(KEY, PAGE)
    return true
  } catch {
    return false
  }
}
