import { useCallback, useEffect, useRef, useSyncExternalStore } from 'react'
import { bugReportUrl } from '@explorer-core/lib/bugReport'
import { isNativeHud } from '@hud-host'
import type { EngineSession } from '../session/useEngineSession'
import { useCommandResult, type DebugSection } from './DebugPanel'

const sessionSection: DebugSection = {
  title: 'Session', query: 'hud/session', fields: [['Phase', 'phase'], ['Engine ready', 'ready']],
  actions: [['Open lobby', 'hud/lobby'], ['Sign out and open sign-in', 'hud/signout'], ['Report a bug', 'hud/report']]
}

const REQUEST = 'dcl-debug-panel'
type Request = { target: string; report: (message: string) => void }

/** The open panel: `target` is the section it opened on, `opened` increases on each open. */
export type DebugPanelState = { target: string; opened: number } | null

// Module-level store, like the popup stack: the open state outlives every component that reads it.
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
        return JSON.stringify([...sections, sessionSection])
      }
      case 'hud/session': return JSON.stringify({ phase: s.phase, ready: s.login.engineReady })
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
  const { value: developmentBuild } = useCommandResult<boolean>(run, '/debug_panel --dev-build', false)
  return { state, run, developmentBuild: developmentBuild === true }
}
