import { useCallback, useEffect, useRef, useState } from 'react'
import { bugReportUrl } from '../../lib/bugReport'
import { isNativeHud } from '@hud-host'
import type { EngineSession } from '../session/useEngineSession'
import { openDebugPanel, useCommandResult, type DebugSection } from './DebugPanel'

const sessionSection: DebugSection = {
  title: 'Session', query: 'hud/session', fields: [['Phase', 'phase'], ['Engine ready', 'ready']],
  actions: [['Open lobby', 'hud/lobby'], ['Sign out and open sign-in', 'hud/signout'], ['Report a bug', 'hud/report']]
}

const REQUEST = 'dcl-debug-panel'
type Request = { target: string; report: (message: string) => void }

/** The chat entry point uses the same in-world panel as the sidebar. */
export function requestDebugPanel(target: string, report: Request['report']) {
  window.dispatchEvent(new CustomEvent<Request>(REQUEST, { detail: { target, report } }))
}

export function useDebugPanel(session: EngineSession) {
  const current = useRef(session)
  current.current = session
  const popup = useRef<(() => void) | null>(null)
  const [launcherLayer, setLauncherLayer] = useState<HTMLDivElement | null>(null)
  const run = useCallback(async (line: string) => {
    const s = current.current
    switch (line) {
      case '/debug_panel': {
        const sections: DebugSection[] = JSON.parse(await s.runDebugCommand(line))
        return JSON.stringify([...sections, sessionSection])
      }
      case 'hud/session': return JSON.stringify({ phase: s.phase, ready: s.login.engineReady })
      case 'hud/lobby': popup.current?.(); s.lobbyPage.toggle(); return 'Opened lobby'
      case 'hud/signout': popup.current?.(); s.logout(); return 'Signed out'
      case 'hud/report':
        if (isNativeHud()) s.nav('support')
        else window.open(bugReportUrl(), '_blank', 'noopener')
        return 'Requested the support page in your browser'
      default: return s.runDebugCommand(line)
    }
  }, [])
  const { value: developmentBuild } = useCommandResult<boolean>(run, '/debug_panel --dev-build', false)
  const toggle = useCallback((target = '') => {
    const alreadyOpen = popup.current != null
    popup.current?.()
    if (!alreadyOpen || target) popup.current = openDebugPanel(run, target, () => { popup.current = null }, setLauncherLayer)
  }, [run])
  useEffect(() => {
    const onRequest = (event: Event) => {
      const { target, report } = (event as CustomEvent<Request>).detail
      if (target.toLowerCase() === 'help') {
        void run('/debug_panel').then((json) => report((JSON.parse(json) as DebugSection[]).map(s => s.title).join('\n')))
          .catch((error: unknown) => report(String(error)))
      } else toggle(target)
    }
    window.addEventListener(REQUEST, onRequest)
    return () => { window.removeEventListener(REQUEST, onRequest); popup.current?.() }
  }, [run, toggle])
  return { toggle, developmentBuild: developmentBuild === true, launcherLayer }
}
