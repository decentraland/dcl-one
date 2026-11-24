import { useId, useState, type ReactNode } from 'react'
import { InputField } from '../../../components/InputField/InputField'
import { Switch } from '../../../components/Switch/Switch'
import { GeneralClose } from '../../../foundations/icons'
import { Button } from '../shared/Button/Button'
import { ADMIN_PERMISSIONS, adminCopy } from './admin-copy'
import { Dialog } from './Dialog'
import styles from './AdminModal.module.css'

const WALLET = /^0x[0-9a-fA-F]{40}$/
const SELF_LOCKED = 'edit_any_profile'

const shortAddress = (value: string) =>
  value.length > 12 ? `${value.slice(0, 6)}…${value.slice(-4)}` : value

export type PermissionsModalProps = {
  /** The account being edited; none to add one by its wallet address. */
  user?: { address: string; name?: string; avatarUrl?: string }
  /** The account's current grants. Grants this dialog does not show are kept as they are. */
  permissions: readonly string[]
  /**
   * The grants of a listed account, by wallet. A typed address that names one turns Add into Edit of
   * that account, with its grants, because a save replaces the account's whole set.
   */
  existing?: (address: string) => readonly string[] | undefined
  /** The signed-in account: it may not take its own user administration away. */
  self?: string
  busy: boolean
  feedback?: ReactNode
  onClose: () => void
  /**
   * `permissions` are the chosen grants and `initial` the grants the switches started from (the edited
   * account's, a typed listed account's, or none for a fresh Add). Their difference is the change to apply
   * to the account's current set, which may have moved since the list was read.
   */
  onSubmit: (payload: { address: string; permissions: string[]; initial: string[] }) => void
}

/**
 * decentraland.org's AdminPermissionsModal: Add User asks for a wallet address, Edit User shows the
 * account; a switch per grant, Cancel and Save Changes (enabled for a valid address).
 */
export function PermissionsModal({
  user,
  permissions: initial,
  existing,
  self,
  busy,
  feedback,
  onClose,
  onSubmit
}: PermissionsModalProps) {
  const titleId = useId()
  const descriptionPrefix = useId()
  const [address, setAddress] = useState(user?.address ?? '')
  const [permissions, setPermissions] = useState<string[]>([...initial])
  // The choices made before the typed address named a listed account, restored when it no longer does,
  // and that account's grants, which the switches then started from.
  const [beforeMatch, setBeforeMatch] = useState<string[] | null>(null)
  const [matched, setMatched] = useState<readonly string[] | null>(null)
  const start = matched ?? initial
  const valid = WALLET.test(address.trim())
  const invalid = address.length > 0 && !valid
  const target = user?.address ?? (valid ? address.trim() : '')
  const selfEdit = Boolean(target && self && target.toLowerCase() === self.toLowerCase())
  const editing = Boolean(user) || beforeMatch !== null
  const typeAddress = (next: string) => {
    setAddress(next)
    const held = WALLET.test(next.trim()) ? existing?.(next.trim()) : undefined
    if (held) {
      if (beforeMatch === null) setBeforeMatch(permissions)
      setMatched(held)
      setPermissions([...held])
    } else if (beforeMatch !== null) {
      setPermissions(beforeMatch)
      setBeforeMatch(null)
      setMatched(null)
    }
  }
  // The signed-in account keeps its own user administration, whatever the dialog started from.
  const submitted =
    selfEdit && !permissions.includes(SELF_LOCKED) ? [...permissions, SELF_LOCKED] : permissions
  const close = () => {
    if (!busy) onClose()
  }
  return (
    <Dialog labelledBy={titleId} onClose={close} maxWidth={621}>
      <div className={styles.panel}>
        <div className={styles.header}>
          <h2 id={titleId} className={styles.title}>
            {editing ? adminCopy.permissions.editTitle : adminCopy.permissions.addTitle}
          </h2>
          <button
            type="button"
            className={styles.close}
            aria-label={adminCopy.permissions.close}
            disabled={busy}
            onClick={close}
          >
            <GeneralClose size={24} aria-hidden="true" />
          </button>
        </div>
        {user ? (
          <div className={styles.identity}>
            {user.avatarUrl ? (
              <img className={styles.avatar} src={user.avatarUrl} alt="" />
            ) : (
              <span className={styles.avatar} aria-hidden="true" />
            )}
            <div className={styles.identityText}>
              <span className={styles.identityName}>{user.name || shortAddress(user.address)}</span>
              <span className={styles.identityAddress}>{user.address}</span>
            </div>
          </div>
        ) : (
          <InputField
            surface="web"
            label={adminCopy.permissions.walletLabel}
            placeholder={adminCopy.permissions.walletPlaceholder}
            value={address}
            disabled={busy}
            autoComplete="off"
            spellCheck={false}
            error={invalid ? adminCopy.permissions.walletInvalid : undefined}
            onChange={(event) => typeAddress(event.target.value)}
          />
        )}
        <ul className={styles.permissions}>
          {ADMIN_PERMISSIONS.map((permission) => {
            const locked = selfEdit && permission.id === SELF_LOCKED
            const descriptionId = `${descriptionPrefix}-${permission.id}`
            return (
              <li key={permission.id} className={styles.permission}>
                <div className={styles.permissionText}>
                  <p className={styles.permissionTitle} aria-hidden="true">
                    {permission.title}
                  </p>
                  <p id={descriptionId} className={styles.permissionDescription}>
                    {locked ? adminCopy.permissions.selfEditLocked : permission.description}
                  </p>
                </div>
                <Switch
                  label={permission.title}
                  labelHidden
                  aria-describedby={descriptionId}
                  checked={submitted.includes(permission.id)}
                  disabled={busy || locked}
                  onCheckedChange={(checked) =>
                    setPermissions((current) =>
                      checked
                        ? [...current, permission.id]
                        : current.filter((value) => value !== permission.id)
                    )
                  }
                />
              </li>
            )
          })}
        </ul>
        {feedback}
        <div className={styles.footer}>
          <Button variant="secondary" disabled={busy} onClick={close}>
            {adminCopy.permissions.cancel}
          </Button>
          <Button
            disabled={!valid || busy}
            onClick={() =>
              onSubmit({ address: address.trim(), permissions: submitted, initial: [...start] })
            }
          >
            {adminCopy.permissions.save}
          </Button>
        </div>
      </div>
    </Dialog>
  )
}
