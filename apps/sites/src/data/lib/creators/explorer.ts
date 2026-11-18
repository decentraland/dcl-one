import { Env } from '@dcl/ui-env'
import { config } from '~/config'
import { track } from './analytics'
import { openExternal, openProtocolLink } from './navigation'

type LaunchOptions = {
  /** How long to wait for the tab to lose focus before assuming no desktop client is installed. */
  timeoutMs?: number
}

function resolveDclEnv(): 'zone' | 'today' | 'org' {
  if (config.is(Env.DEVELOPMENT)) return 'zone'
  if (config.is(Env.STAGING)) return 'today'
  return 'org'
}

/**
 * `decentraland://` deep link that cold-starts the desktop explorer with an unreleased builder
 * collection injected. The client fetches it from the builder API with the logged-in wallet, lands
 * in the per-environment preview world and opens the backpack right away.
 */
export function buildCollectionPreviewDeepLink(collectionId: string): string {
  const params = new URLSearchParams()
  params.set('self-preview-builder-collections', collectionId)
  params.set('dclenv', resolveDclEnv())
  params.set('realm', config.get('PREVIEW_WORLD'))
  params.set('force-open-backpack', 'true')
  return `decentraland://?${params.toString()}`
}

function isElectronApp(): boolean {
  const w = window as Window & { process?: { type?: string } }
  return w.process?.type === 'renderer' || navigator.userAgent.includes('Electron')
}

/**
 * Opens the desktop explorer on the collection preview. Must run synchronously inside a user
 * gesture so the browser allows the protocol navigation. Resolves `true` when the tab loses focus
 * within the timeout (the OS routed the scheme to an installed client), `false` otherwise.
 */
export function launchCollectionPreview(
  collectionId: string,
  { timeoutMs = 750 }: LaunchOptions = {}
): Promise<boolean> {
  if (typeof window === 'undefined' || isElectronApp()) return Promise.resolve(false)

  let opened = false
  const onLoseFocus = () => {
    opened = true
  }
  const onVisibilityChange = () => {
    if (document.visibilityState === 'hidden') onLoseFocus()
  }
  const cleanup = () => {
    document.removeEventListener('visibilitychange', onVisibilityChange)
    window.removeEventListener('pagehide', onLoseFocus)
    window.removeEventListener('blur', onLoseFocus)
  }

  document.addEventListener('visibilitychange', onVisibilityChange, { passive: true })
  window.addEventListener('pagehide', onLoseFocus, { passive: true })
  window.addEventListener('blur', onLoseFocus, { passive: true })

  try {
    openProtocolLink(buildCollectionPreviewDeepLink(collectionId))
  } catch {
    cleanup()
    return Promise.resolve(false)
  }

  return new Promise(resolve => {
    setTimeout(() => {
      cleanup()
      resolve(opened)
    }, timeoutMs)
  })
}

/**
 * Jumps into the collection preview, or sends the user to get the desktop app when none is installed.
 * Resolves whether the desktop client took over.
 */
export async function previewCollection(collectionId: string): Promise<boolean> {
  const launched = await launchCollectionPreview(collectionId)
  if (!launched) openExternal(config.get('DOWNLOAD_URL'))
  return launched
}

const launching = new Set<string>()

/** Launches the preview from a collection page and tracks it; a click while that launch is settling is ignored. */
export function previewCollectionFromDetail(collectionId: string, itemCount: number): void {
  if (launching.has(collectionId)) return
  launching.add(collectionId)
  void previewCollection(collectionId)
    .then(launched => track('Preview In World', { collectionId, itemCount, launched, trigger: 'detail' }))
    .finally(() => launching.delete(collectionId))
}
