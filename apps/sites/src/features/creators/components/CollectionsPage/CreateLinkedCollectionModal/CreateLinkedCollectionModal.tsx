import { useState } from 'react'
import { config } from '~/config'
import { useTranslation } from '~/intl'
import { openExternal } from '~/lib/navigation'
import { isContractInUseError } from '~/lib/linkedContract'
import {
  LINKED_COLLECTION_NAME_MAX_LENGTH,
  LINKED_WEARABLES_DOCS_PATH,
  validateLinkedCollectionName,
  type LinkedContract
} from '~/lib/thirdParty'
import { Button } from '~/components/Button'
import { CollectionNameInput } from '~/components/CollectionNameInput'
import { EMPTY_LINKED_CONTRACT, LinkedContractFields } from '~/components/LinkedContractFields'
import { Modal } from '~/components/Modal'
import nameInput from '~/components/CollectionNameInput/CollectionNameInput.module.css'
import styles from './CreateLinkedCollectionModal.module.css'
import nameModal from '~/components/CollectionNameModal/CollectionNameModal.module.css'
import shared from '~/styles/shared.module.css'

type Props = {
  isPending: boolean
  /** The failed save, mapped to friendly copy here. */
  error: unknown
  onSubmit: (name: string, contract: LinkedContract) => void
  onClose: () => void
}

/** The New Linked Collection dialog: a name, the NFT contract's network and address, checked on chain as typed. */
export function CreateLinkedCollectionModal({ isPending, error, onSubmit, onClose }: Props) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [contract, setContract] = useState(EMPTY_LINKED_CONTRACT)
  const { network, address, check } = contract
  const [nameError, setNameError] = useState<string | null>(null)
  const [editedSinceSubmit, setEditedSinceSubmit] = useState(false)

  const learnMoreUrl = `${config.get('DOCS_URL')}${LINKED_WEARABLES_DOCS_PATH}`

  const trimmed = name.trim()
  const canSubmit = !!trimmed && !!network && check === 'ok' && !isPending

  function handleSubmit(event: React.FormEvent) {
    event.preventDefault()
    if (!canSubmit || !network) return
    if (validateLinkedCollectionName(trimmed)) {
      setNameError(t('collection_name_modal.error_invalid_character'))
      return
    }
    setNameError(null)
    setEditedSinceSubmit(false)
    onSubmit(trimmed, { network, address })
  }

  const serverError =
    error && !editedSinceSubmit
      ? isContractInUseError(error)
        ? t('create_linked_collection_modal.error_contract_in_use')
        : t('create_linked_collection_modal.error_generic')
      : null
  return (
    <Modal
      title={t('create_linked_collection_modal.title')}
      onClose={onClose}
      closeDisabled={isPending}
      testId="create-linked-collection-modal"
    >
      <form className={nameModal.form} onSubmit={handleSubmit}>
        <div className={nameModal.intro}>
          <h3 className={nameModal.heading}>{t('create_linked_collection_modal.heading')}</h3>
          <p className={nameModal.subtitle}>
            {t('create_linked_collection_modal.subtitle')}{' '}
            <a
              className={styles.learnMore}
              href={learnMoreUrl}
              data-testid="linked-learn-more"
              onClick={event => {
                event.preventDefault()
                openExternal(learnMoreUrl)
              }}
            >
              {t('create_linked_collection_modal.learn_more')}
            </a>
          </p>
        </div>
        <div className={styles.fields}>
          <CollectionNameInput
            value={name}
            error={nameError}
            placeholder={t('create_linked_collection_modal.name_placeholder')}
            maxLength={LINKED_COLLECTION_NAME_MAX_LENGTH}
            hideHint
            autoFocus
            testId="linked-collection-name"
            onChange={value => {
              setName(value)
              setNameError(null)
              setEditedSinceSubmit(true)
            }}
          />
          <LinkedContractFields
            value={contract}
            onChange={next => {
              setContract(next)
              if (next.network !== network || next.address !== address) setEditedSinceSubmit(true)
            }}
          />
          {serverError && (
            <p className={nameInput.errorText} data-testid="linked-collection-error">
              {serverError}
            </p>
          )}
        </div>
        <div className={shared.modalActions}>
          <Button
            type="button"
            variant="secondary"
            data-testid="linked-collection-cancel"
            disabled={isPending}
            onClick={onClose}
          >
            {t('collection_name_modal.cancel')}
          </Button>
          <Button type="submit" data-testid="linked-collection-submit" loading={isPending} disabled={!canSubmit}>
            {t('create_linked_collection_modal.submit')}
          </Button>
        </div>
      </form>
    </Modal>
  )
}
