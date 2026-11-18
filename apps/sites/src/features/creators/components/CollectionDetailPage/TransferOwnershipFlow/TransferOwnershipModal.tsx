import { ChevronLeftIcon, ChevronRightIcon, WarningAmberOutlinedIcon as WarningIcon } from '../../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { type Collection } from '@/data/lib/creators/collections'
import { getNewOwnerError } from '@/data/lib/creators/collectionOwnership'
import { type Friend } from '@/data/lib/creators/friends'
import { getMaticChainId } from '@/data/lib/creators/publishCollection'
import { Button } from '../../Button'
import { Modal } from '../../Modal'
import { StepIndicator } from '../../StepIndicator'
import { BeneficiaryInput } from '../SellItemFlow/BeneficiaryInput'
import { cx } from '@/data/lib/creators/cx'
import sell from '../SellItemFlow/SellItemModal.module.css'
import send from '../SendItemsFlow/SendItemsModal.module.css'
import styles from './TransferOwnershipModal.module.css'
import roles from '../ManageRolesFlow/ManageRolesModal.module.css'
import { RecipientLine } from '../SendItemsFlow/SendItemsModal'

export type TransferStep = 'details' | 'confirm'

type Props = {
  collection: Collection
  step: TransferStep
  /** The chosen new owner (lowercased), or '' while none is chosen. */
  newOwner: string
  /** What was typed to confirm; must match the collection's name. */
  confirmation: string
  friends: Friend[] | undefined
  isLoadingFriends: boolean
  /** A submit is in flight (custodial wallet, no prompt to wait for): the button spins. */
  busy: boolean
  canContinue: boolean
  onStep: (step: TransferStep) => void
  onNewOwnerChange: (address: string) => void
  onConfirmationChange: (value: string) => void
  onSubmit: () => void
  onClose: () => void
}

/**
 * Two steps, like sending items: pick the new owner and type the collection's name, then read the chosen
 * wallet once more and TRANSFER OWNERSHIP.
 */
export function TransferOwnershipModal({
  collection,
  step,
  newOwner,
  confirmation,
  friends,
  isLoadingFriends,
  busy,
  canContinue,
  onStep,
  onNewOwnerChange,
  onConfirmationChange,
  onSubmit,
  onClose
}: Props) {
  const { t } = useTranslation()

  function rejection(address: string): string | null {
    const error = getNewOwnerError(collection, address, getMaticChainId())
    return error ? t(`transfer_ownership_modal.${error}`) : null
  }

  return (
    <Modal
      title={t('transfer_ownership_modal.title')}
      onClose={onClose}
      closeDisabled={busy}
      compact
      testId="transfer-ownership-modal"
    >
      <hr className={sell.divider} />
      <div className={send.steps}>
        <StepIndicator
          current={step === 'details' ? 1 : 2}
          total={2}
          labels={[t('transfer_ownership_modal.step_details'), t('transfer_ownership_modal.step_confirm')]}
          testId="transfer-steps"
        />
      </div>
      <div
        className={cx(send.body, styles.body)}
        key={step}
        data-busy={busy || undefined}
        aria-busy={busy || undefined}
        inert={busy}
      >
        {step === 'details' ? (
          <>
            <p className={roles.description} data-testid="transfer-ownership-description">
              {t('transfer_ownership_modal.description', { name: collection.name })}
            </p>
            <div className={sell.field}>
              <span className={sell.label}>{t('transfer_ownership_modal.new_owner')}</span>
              <BeneficiaryInput
                value={newOwner}
                onChange={onNewOwnerChange}
                friends={friends}
                isLoadingFriends={isLoadingFriends}
                placeholder={t('transfer_ownership_modal.placeholder')}
                duplicateError={rejection}
                clearLabel={t('transfer_ownership_modal.clear')}
                testId="new-owner"
              />
            </div>
            <div className={sell.field}>
              <span className={sell.label}>
                {t('transfer_ownership_modal.confirm_label', { name: collection.name })}
              </span>
              <div className={sell.box}>
                <input
                  value={confirmation}
                  placeholder={collection.name}
                  autoComplete="off"
                  spellCheck={false}
                  data-testid="transfer-confirmation-input"
                  onChange={event => onConfirmationChange(event.target.value)}
                />
              </div>
            </div>
          </>
        ) : (
          <>
            <p className={send.intro}>{t('transfer_ownership_modal.confirm_intro', { name: collection.name })}</p>
            <div className={sell.field}>
              <span className={sell.label}>{t('transfer_ownership_modal.new_owner')}</span>
              <RecipientLine address={newOwner} friends={friends} testId="transfer-confirm-owner" />
            </div>
            <div className={sell.note} data-variant="warning" data-testid="transfer-ownership-warning">
              <WarningIcon aria-hidden />
              <p>{t('transfer_ownership_modal.warning')}</p>
            </div>
          </>
        )}
      </div>

      <div className={sell.footer}>
        {step === 'details' ? (
          <Button type="button" variant="secondary" data-testid="transfer-cancel" onClick={onClose}>
            {t('sell_item_modal.cancel')}
          </Button>
        ) : (
          <Button
            type="button"
            variant="secondary"
            disabled={busy}
            data-testid="transfer-back"
            onClick={() => onStep('details')}
          >
            <ChevronLeftIcon fontSize="small" />
            {t('send_items_modal.back')}
          </Button>
        )}
        {step === 'details' ? (
          <Button
            type="button"
            variant="primary"
            disabled={!canContinue}
            data-testid="transfer-continue"
            onClick={() => onStep('confirm')}
          >
            {t('send_items_modal.continue')}
            <ChevronRightIcon fontSize="small" />
          </Button>
        ) : (
          <Button
            type="button"
            variant="primary"
            disabled={busy}
            loading={busy}
            data-testid="transfer-submit"
            onClick={onSubmit}
          >
            {t('transfer_ownership_modal.submit')}
          </Button>
        )}
      </div>
    </Modal>
  )
}
