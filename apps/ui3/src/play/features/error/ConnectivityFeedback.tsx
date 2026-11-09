import { useEffect, useRef } from 'react'
import { Button, ModalShell, Panel, openPopup } from '../../design'
import type { ConnectivityState } from '../session/useConnectivity'
import styles from './ConnectivityFeedback.module.css'

const SCOPES: Record<string, string> = { realm: 'Realm room', scene: 'Scene room', private_chat: 'Private chat', voice: 'Voice room' }
const STATES: Record<string, string> = {
  denied: 'access denied', identity_conflict: 'this identity is connected elsewhere',
  invalid: 'connection settings are invalid', credentials_required: 'new credentials are required',
  reconnecting: 'reconnecting', paused: 'paused'
}

/** The engine owns the dialog lifetime; recovery retracts it without signing out. */
export function ConnectivityFeedback({ state, retry, exit }: { state: ConnectivityState | null; retry: () => void; exit: () => void }) {
  const actions = useRef({ retry, exit })
  actions.current = { retry, exit }
  const lost = state?.connectionLost === true
  useEffect(() => {
    if (!lost) return
    return openPopup(() => (
      <ModalShell title="Connection Lost" role="alertdialog" ariaLabel="Connection Lost" closeButton={false}
        actions={<><Button variant="secondary" onClick={() => actions.current.exit()}>Exit</Button><Button onClick={() => actions.current.retry()}>Retry</Button></>}>
        <p>Your connection was interrupted. Reconnecting will continue in the background.</p>
      </ModalShell>
    ), { backdropClickCloses: false, confirmClose: () => false })
  }, [lost])
  const messages = [...new Set(state?.rooms.flatMap(room => {
    const scope = SCOPES[room.scope]
    const reason = STATES[room.state]
    return scope && reason ? [`${scope}: ${reason}`] : []
  }) ?? [])]
  if (lost || messages.length === 0) return null
  return <Panel role="status" aria-label="Multiplayer status" className={styles.status}>{messages.map(message => <div key={message}>{message}</div>)}</Panel>
}
