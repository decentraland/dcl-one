import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { bugReportUrl } from '@explorer-core/lib/bugReport'
import { isNativeHud, serviceUrl, siteRevision } from '@hud-host'
import type { EngineSession } from '../session/useEngineSession'
import type { DebugSection } from './DebugPanel'

const sessionSection: DebugSection = {
  title: 'Session', query: 'hud/session', fields: [['Phase', 'phase'], ['Engine ready', 'ready']],
  actions: [['Open lobby', 'hud/lobby'], ['Sign out and open sign-in', 'hud/signout'], ['Report a bug', 'hud/report']]
}

const deploymentSection = (): DebugSection => ({
  title: 'Deployment', query: 'hud/deployment', poll: false, actions: [],
  fields: [
    ...(siteRevision() === null ? [] : [['Sites', 'sites'] as [string, string]]),
    [`Catalyst (${serviceUrl('catalyst').replace(/^https?:\/\//, '')})`, 'catalyst']
  ]
})

const unreported = (revision: string, by: string): string | undefined =>
  revision === '' || revision.toLowerCase() === 'unknown' ? `not reported ${by}`
    : revision === 'unversioned' ? 'unversioned build' : undefined

async function catalystRevision(): Promise<{ revision: string } | { reason: string }> {
  let body: unknown
  try {
    const response = await fetch(`${serviceUrl('catalyst')}/content/status`, { signal: AbortSignal.timeout(5000) })
    if (!response.ok) {
      void response.body?.cancel().catch(() => {})
      return { reason: `status ${response.status}` }
    }
    body = await response.json()
  } catch (error) {
    const unreadable = (error as { name?: unknown } | null)?.name === 'SyntaxError'
    return { reason: unreadable ? 'answered without a status' : 'no answer from the node' }
  }
  if (!body || typeof body !== 'object') return { reason: 'answered without a status' }
  const revision = (body as { commitHash?: unknown }).commitHash
  if (typeof revision !== 'string') return { reason: 'not reported by the node' }
  const reason = unreported(revision, 'by the node')
  return reason ? { reason } : { revision }
}

async function readDeployment(): Promise<string> {
  const unavailable: Record<string, string> = {}
  const site = siteRevision()
  const siteReason = site === null ? undefined : unreported(site, 'by Sites')
  if (siteReason) unavailable.sites = siteReason
  const catalyst = await catalystRevision()
  if ('reason' in catalyst) unavailable.catalyst = catalyst.reason
  return JSON.stringify({ sites: siteReason ? null : site, catalyst: 'revision' in catalyst ? catalyst.revision : null, unavailable })
}

const REQUEST = 'dcl-debug-panel'
type Request = { target: string; report: (message: string) => void }

/** The open panel: `target` is the section it opened on, `opened` increases on each open. */
export type DebugPanelState = { target: string; opened: number } | null

// Module-level store, like the popup stack: the open state lives outside any component.
let panel: DebugPanelState = null
let opened = 0
const listeners = new Set<() => void>()
const subscribe = (cb: () => void): (() => void) => {
  listeners.add(cb)
  return () => listeners.delete(cb)
}
const setPanel = (next: DebugPanelState): void => {
  panel = next
  listeners.forEach((l) => l())
}

/** Closes an open panel; a named section always opens the panel on that section. */
export function toggleDebugPanel(target = '') {
  setPanel(panel != null && !target ? null : { target, opened: ++opened })
}

export function closeDebugPanel() {
  setPanel(null)
}

/** Read at call time, outside React. */
export function isDebugPanelOpen(): boolean {
  return panel != null
}

export function useDebugPanelState(): DebugPanelState {
  return useSyncExternalStore(subscribe, () => panel)
}

type RunDebugCommand = EngineSession['runDebugCommand']
const developmentBuilds = new WeakMap<RunDebugCommand, Promise<boolean>>()

/** Asks `/debug_panel --dev-build` once per command runner; `false` until it answers or if it fails. */
export function useDevelopmentBuild(runDebugCommand: RunDebugCommand): boolean {
  const [value, setValue] = useState(false)
  useEffect(() => {
    let active = true
    let answer = developmentBuilds.get(runDebugCommand)
    if (answer == null) {
      answer = (async () => JSON.parse(await runDebugCommand('/debug_panel --dev-build')) === true)().catch(() => false)
      developmentBuilds.set(runDebugCommand, answer)
    }
    void answer.then((next) => { if (active) setValue(next) })
    return () => { active = false }
  }, [runDebugCommand])
  return value
}

/** The chat entry point uses the same in-world panel as the sidebar. */
export function requestDebugPanel(target: string, report: Request['report']) {
  window.dispatchEvent(new CustomEvent<Request>(REQUEST, { detail: { target, report } }))
}

export function useDebugPanel(session: EngineSession) {
  const current = useRef(session)
  current.current = session
  const state = useDebugPanelState()
  const run = useCallback(async (line: string) => {
    const s = current.current
    switch (line) {
      case '/debug_panel': {
        const sections: DebugSection[] = JSON.parse(await s.runDebugCommand(line))
        return JSON.stringify([...sections, sessionSection, deploymentSection()])
      }
      case 'hud/session': return JSON.stringify({ phase: s.phase, ready: s.login.engineReady })
      case 'hud/deployment': return readDeployment()
      case 'hud/lobby': closeDebugPanel(); s.lobbyPage.toggle(); return 'Opened lobby'
      case 'hud/signout': closeDebugPanel(); s.logout(); return 'Signed out'
      case 'hud/report':
        if (isNativeHud()) s.nav('support')
        else window.open(bugReportUrl(), '_blank', 'noopener')
        return 'Requested the support page in your browser'
      default: return s.runDebugCommand(line)
    }
  }, [])
  useEffect(() => {
    const onRequest = (event: Event) => {
      const { target, report } = (event as CustomEvent<Request>).detail
      if (target.toLowerCase() === 'help') {
        void run('/debug_panel').then((json) => report((JSON.parse(json) as DebugSection[]).map(s => s.title).join('\n')))
          .catch((error: unknown) => report(String(error)))
      } else toggleDebugPanel(target)
    }
    window.addEventListener(REQUEST, onRequest)
    return () => { window.removeEventListener(REQUEST, onRequest); closeDebugPanel() }
  }, [run])
  return { state, run, developmentBuild: useDevelopmentBuild(session.runDebugCommand) }
}
