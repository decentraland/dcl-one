import { useState } from 'react'
import { AddIcon } from '~/components/Icons'
import { useTranslation } from '~/intl'
import { getRoleAddressError, type RoleKind } from '~/lib/collectionRoles'
import { type Friend } from '~/lib/friends'
import { getMaticChainId } from '~/lib/publishCollection'
import { Button } from '~/components/Button'
import { Modal } from '~/components/Modal'
import { BeneficiaryInput } from '../SellItemFlow/BeneficiaryInput'
import { cx } from '~/lib/cx'
import sell from '../SellItemFlow/SellItemModal.module.css'
import send from '../SendItemsFlow/SendItemsModal.module.css'
import styles from './ManageRolesModal.module.css'

type Props = {
  kind: RoleKind
  /** The signed-in owner's address, lowercased: offered as "You" so they can keep a role after a transfer. */
  self: string
  /** The draft list, lowercased. */
  addresses: string[]
  friends: Friend[] | undefined
  isLoadingFriends: boolean
  /** A submit is in flight (custodial wallet, no prompt to wait for): the button spins. */
  busy: boolean
  canSave: boolean
  onAdd: (address: string) => void
  /** Drops an address from the draft; nothing changes on-chain until the list is saved. */
  onRemove: (address: string) => void
  onSubmit: () => void
  onClose: () => void
}

/** The role's address list: one chip per holder, a picker row to add another, SAVE CHANGES once the list differs. */
export function ManageRolesModal({
  kind,
  self,
  addresses,
  friends,
  isLoadingFriends,
  busy,
  canSave,
  onAdd,
  onRemove,
  onSubmit,
  onClose
}: Props) {
  const { t } = useTranslation()
  const [adding, setAdding] = useState(false)
  const showInput = addresses.length === 0 || adding

  function rejection(address: string): string | null {
    const error = getRoleAddressError(kind, addresses, address, getMaticChainId())
    return error ? t(`manage_roles_modal.${error}`) : null
  }

  return (
    <Modal
      title={t(`manage_roles_modal.${kind}.title`)}
      onClose={onClose}
      closeDisabled={busy}
      compact
      testId="manage-roles-modal"
    >
      <hr className={sell.divider} />
      <div
        className={send.body}
        data-busy={busy || undefined}
        aria-busy={busy || undefined}
        {...(busy ? { inert: '' } : {})}
      >
        <p className={styles.description} data-testid="manage-roles-description">
          {t(`manage_roles_modal.${kind}.description`)}
        </p>
        <ul className={styles.list}>
          {addresses.map((address, index) => (
            <li className={styles.row} key={address} data-testid="role-row" data-address={address}>
              <BeneficiaryInput
                value={address}
                onChange={() => onRemove(address)}
                friends={friends}
                self={self}
                isLoadingFriends={isLoadingFriends}
                clearLabel={t(`manage_roles_modal.${kind}.remove`)}
                variant="compact"
                testId={`role-${index + 1}`}
              />
            </li>
          ))}
          {showInput && (
            <li className={styles.row}>
              <BeneficiaryInput
                value=""
                onChange={address => {
                  onAdd(address)
                  setAdding(false)
                }}
                friends={friends}
                self={self}
                isLoadingFriends={isLoadingFriends}
                placeholder={t(`manage_roles_modal.${kind}.placeholder`)}
                duplicateError={rejection}
                testId="role-new"
              />
            </li>
          )}
        </ul>
        <button
          className={cx(send.linkButton, styles.linkButton)}
          type="button"
          disabled={showInput}
          data-testid="role-add"
          onClick={() => setAdding(true)}
        >
          <AddIcon />
          {t(`manage_roles_modal.${kind}.add`)}
        </button>
      </div>

      <div className={sell.footer}>
        <Button type="button" variant="secondary" disabled={busy} data-testid="role-cancel" onClick={onClose}>
          {t('sell_item_modal.cancel')}
        </Button>
        <Button
          type="button"
          variant="primary"
          disabled={!canSave}
          loading={busy}
          data-testid="role-save"
          onClick={onSubmit}
        >
          {t('manage_roles_modal.save')}
        </Button>
      </div>
    </Modal>
  )
}
