// Smart wearable authorization: a smart wearable (a wearable carrying an SDK7 scene) the player
// logged in wearing or just equipped asks to run. The engine relays the request over the bridge
// (domains/smartWearables.ts) and waits for the answer; it asks one at a time. The scene permission
// dialog's card, with the wearable's declared permissions in that dialog's wording.

import { Button, openPopup } from '../../design'
import type { SmartWearableAuthRequestMessage } from '../../engine/protocol'
import { LockIcon, describePermission } from './PermissionDialog'
import styles from './SmartWearableAuthorizationDialog.module.css'

// scene.json permission name → the PermissionType whose wording the scene permission dialog uses.
const PERMISSION_TYPE: Record<string, string> = {
  USE_FETCH: 'Fetch',
  USE_WEBSOCKET: 'Websocket',
  USE_WEB3_API: 'Web3',
  OPEN_EXTERNAL_LINK: 'OpenUrl'
}

/** A declared permission in words; a name the HUD does not know is shown as declared. */
function describeDeclared(name: string): string {
  const ty = PERMISSION_TYPE[name]
  return ty == null ? name : describePermission(ty)
}

export function SmartWearableAuthorizationDialog({
  request,
  onAnswer
}: {
  request: SmartWearableAuthRequestMessage
  onAnswer: (allow: boolean) => void
}): React.JSX.Element {
  const name = <span className={styles.name}>{request.name}</span>
  return (
    <div className={styles.modal} role="dialog" aria-modal="true" aria-label="Smart wearable permissions" onClick={(e) => e.stopPropagation()}>
      <span className={styles.icon}>
        <LockIcon />
      </span>
      <div className={styles.prompt}>The smart wearable {name} wants to run while you wear it</div>
      <div className={styles.reason}>{request.reason === 'login' ? <>You logged in wearing {name}.</> : <>You equipped {name}.</>}</div>
      {request.permissions.length > 0 && (
        <div className={styles.permissions}>
          <div className={styles.heading}>It asks permission to</div>
          <ul className={styles.list}>
            {request.permissions.map((p) => (
              <li key={p}>{describeDeclared(p)}</li>
            ))}
          </ul>
        </div>
      )}
      <div className={styles.actions}>
        <Button variant="ghost" className={styles.deny} onClick={() => onAnswer(false)}>
          Deny
        </Button>
        <Button variant="primary" onClick={() => onAnswer(true)}>
          Allow
        </Button>
      </div>
    </div>
  )
}

/** Open the authorization prompt as a popup; returns the close handle. Escape / scrim-click deny (a
 *  smart wearable never runs without a yes). Answers exactly once. */
export function openSmartWearableAuthorization(
  request: SmartWearableAuthRequestMessage,
  answer: (id: number, allow: boolean) => void
): () => void {
  let settled = false
  const done = (allow: boolean): void => {
    if (settled) return
    settled = true
    answer(request.id, allow)
  }
  return openPopup(
    (close) => (
      <SmartWearableAuthorizationDialog
        request={request}
        onAnswer={(allow) => {
          done(allow)
          close()
        }}
      />
    ),
    { onClose: () => done(false) }
  )
}
