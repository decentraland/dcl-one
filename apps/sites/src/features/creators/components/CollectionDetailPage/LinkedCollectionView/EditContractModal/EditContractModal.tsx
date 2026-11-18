import { useState } from 'react'
import { useTranslation } from '~/intl'
import { type LinkedContract } from '~/lib/thirdParty'
import { Button } from '~/components/Button'
import { EMPTY_LINKED_CONTRACT, LinkedContractFields } from '~/components/LinkedContractFields'
import { Modal } from '~/components/Modal'
import nameModal from '~/components/CollectionNameModal/CollectionNameModal.module.css'
import shared from '~/styles/shared.module.css'

type Props = {
  /** The contract a previous attempt submitted, so a retry starts from it. */
  initial?: LinkedContract
  /** While the change runs for a social login, whose wallet never prompts: the submit button spins instead. */
  isPending: boolean
  onSubmit: (contract: LinkedContract) => void
  onClose: () => void
}

/**
 * Re-points a linked collection to another NFT contract (a redeploy after a hack, a migration), network included.
 * Submitting the contract the collection already names is allowed: it finishes a change that stopped halfway.
 */
export function EditContractModal({ initial, isPending, onSubmit, onClose }: Props) {
  const { t } = useTranslation()
  const [contract, setContract] = useState(initial ? { ...initial, check: null } : EMPTY_LINKED_CONTRACT)
  const { network, address, check } = contract
  const canSubmit = !!network && check === 'ok' && !isPending

  function handleSubmit(event: React.FormEvent) {
    event.preventDefault()
    if (!canSubmit || !network) return
    onSubmit({ network, address: address.toLowerCase() })
  }

  return (
    <Modal
      title={t('edit_contract_modal.title')}
      onClose={onClose}
      closeDisabled={isPending}
      testId="edit-contract-modal"
    >
      <form className={nameModal.form} onSubmit={handleSubmit}>
        <div className={nameModal.intro}>
          <h3 className={nameModal.heading}>{t('edit_contract_modal.heading')}</h3>
          <p className={nameModal.subtitle}>{t('edit_contract_modal.subtitle')}</p>
        </div>
        <LinkedContractFields value={contract} onChange={setContract} />
        <div className={shared.modalActions}>
          <Button
            type="button"
            variant="secondary"
            data-testid="edit-contract-cancel"
            disabled={isPending}
            onClick={onClose}
          >
            {t('collection_name_modal.cancel')}
          </Button>
          <Button type="submit" data-testid="edit-contract-submit" loading={isPending} disabled={!canSubmit}>
            {t('edit_contract_modal.submit')}
          </Button>
        </div>
      </form>
    </Modal>
  )
}
