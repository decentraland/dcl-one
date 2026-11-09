import { useEffect, useRef } from 'react'
import { Button, ModalShell, Panel, openPopup } from '../../design'
import type { ConnectivityState } from '../session/useConnectivity'
import connectionLostIcon from './connection-lost.png'
import floorLight from './modal-light.png'
import styles from './ConnectivityFeedback.module.css'

const SCOPES: Record<string, string> = { realm: 'Realm room', scene: 'Scene room', private_chat: 'Private chat', voice: 'Voice room' }
const STATES: Record<string, string> = {
  denied: 'access denied', identity_conflict: 'this identity is connected elsewhere',
  invalid: 'connection settings are invalid', credentials_required: 'new credentials are required',
  reconnecting: 'reconnecting', paused: 'paused'
}

/** The engine owns the dialog lifetime; recovery retracts it without signing out. */
export function ConnectivityFeedback({ state, retry, exit }: { state: ConnectivityState | null; retry: () => void | Promise<void>; exit: () => void }) {
  const actions = useRef({ retry, exit })
  actions.current = { retry, exit }
  const lost = state?.connectionLost === true
  useEffect(() => {
    if (!lost) return
    // Unity ErrorPopupWithRetry.prefab: 600×417 card, 396×277 centered content.
    // The owner handle bypasses the Escape guard when Retry/Exit is chosen.
    const dismiss = openPopup(() => (
      <ModalShell width={600} className={styles.modal} bodyless role="alertdialog" ariaLabel="Connection Lost" closeButton={false}>
        <div className={styles.content}>
          <img className={styles.floor} src={floorLight} alt="" />
          <img className={styles.icon} src={connectionLostIcon} alt="" />
          <h2 className={styles.title}>Connection Lost</h2>
          <p className={styles.description}>You appear to be offline. Please check your internet connection and retry.</p>
          <div className={styles.actions}>
            <Button variant="secondary" className={styles.exit} onClick={() => { dismiss(); actions.current.exit() }}>Exit Application</Button>
            <Button className={styles.retry} onClick={async () => {
              try { await actions.current.retry(); dismiss() }
              catch { /* Keep Exit/Retry available if the engine cannot accept the request. */ }
            }}>Retry</Button>
          </div>
        </div>
      </ModalShell>
    ), { className: styles.backdrop, backdropClickCloses: false, confirmClose: () => false })
    return dismiss
  }, [lost])
  const messages = [...new Set(state?.rooms.flatMap(room => {
    const scope = SCOPES[room.scope]
    const reason = STATES[room.state]
    return scope && reason ? [`${scope}: ${reason}`] : []
  }) ?? [])]
  if (lost || messages.length === 0) return null
  return <Panel role="status" aria-label="Multiplayer status" className={styles.status}>{messages.map(message => <div key={message}>{message}</div>)}</Panel>
}
